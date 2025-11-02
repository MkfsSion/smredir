use crate::ccid::CCIDInterfaceHandler;
use crate::device::ControlSetup;
use log::{debug, error};
use nusb::MaybeFuture;
use nusb::transfer;
use std::any::Any;
use std::fmt::{Debug, Formatter};
use std::io;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use usbip::{
    ClassCode, DescriptorType, SetupPacket, StandardRequest, UsbEndpoint, UsbInterface,
    UsbInterfaceHandler,
};

pub struct WebUSBInterfaceHandler {
    device: nusb::Device,
    interface: nusb::Interface,
    interface_number: u8,
    ccid: Arc<Mutex<Box<dyn UsbInterfaceHandler + Send>>>,
}

impl Debug for WebUSBInterfaceHandler {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "WebUSBInterfaceHandler")
    }
}

impl WebUSBInterfaceHandler {
    pub fn new(
        device: nusb::Device,
        interface_number: u8,
        ccid: Arc<Mutex<Box<dyn UsbInterfaceHandler + Send>>>,
    ) -> Result<Self, io::Error> {
        let webusb = device
            .active_configuration()
            .map_err(io::Error::from)?
            .interfaces()
            .find(|interface| {
                interface
                    .alt_settings()
                    .any(|setting| setting.class() == ClassCode::VendorSpecific as u8)
            })
            .ok_or(io::Error::new(
                io::ErrorKind::NotFound,
                "No vendor specific interface found on USB device".to_string(),
            ))?;
        let interface = device
            .claim_interface(webusb.interface_number())
            .wait()
            .map_err(|e| io::Error::new(io::ErrorKind::ResourceBusy, e))?;
        Ok(Self {
            device,
            interface,
            interface_number,
            ccid,
        })
    }
}

fn control_string(control: &ControlSetup) -> String {
    match control {
        ControlSetup::In(control) => {
            format!(
                "ControlIn: control_type: {:0X?}, recipient: {:0X?}, request: {:0X}, value: {:0X}, index: {:0X}",
                control.control_type,
                control.recipient,
                control.request,
                control.value,
                control.index
            )
        }
        ControlSetup::Out(control) => {
            format!(
                "ControlOut: control_type: {:0X?}, recipient: {:0X?}, request: {:0X}, value: {:0X}, index: {:0X}, data: {:02X?}",
                control.control_type,
                control.recipient,
                control.request,
                control.value,
                control.index,
                control.data
            )
        }
    }
}
impl UsbInterfaceHandler for WebUSBInterfaceHandler {
    fn handle_device_urb(
        &mut self,
        transfer_buffer_length: u32,
        setup: SetupPacket,
        req: &[u8],
    ) -> io::Result<Vec<u8>> {
        let control = ControlSetup::new(&setup, Some(req))?;
        match control {
            ControlSetup::In(control) => {
                let mut data = self
                    .interface
                    .control_in(control, Duration::from_secs(5))
                    .wait()
                    .map_err(io::Error::from)?;
                if data.len() > transfer_buffer_length as usize {
                    data.truncate(transfer_buffer_length as usize);
                }
                Ok(data)
            }
            ControlSetup::Out(control) => {
                self.interface
                    .control_out(control, Duration::from_secs(5))
                    .wait()
                    .map_err(io::Error::from)?;
                Ok(vec![])
            }
        }
    }

    fn get_device_capability_descriptors(&self) -> Vec<Vec<u8>> {
        let bos = match self
            .device
            .get_descriptor(DescriptorType::BOS as u8, 0, 0, Duration::from_secs(1))
            .wait()
        {
            Ok(bos) => bos,
            Err(e) => {
                error!("Failed to get BOS descriptor from USB device: {}", e);
                return Vec::new();
            }
        };
        if bos.len() < 5 || bos[0] != 0x5 || bos[1] != DescriptorType::BOS as u8 {
            error!("Invalid BOS descriptor from USB device");
            return Vec::new();
        }
        let total_length = bos[2] as u16 | ((bos[3] as u16) << 8);
        let num_capabilities = bos[4];
        if num_capabilities == 0 {
            error!(
                "BOS descriptor returned by device indicates no device capability descriptor present"
            );
            return Vec::new();
        }
        if total_length as usize != bos.len() {
            error!(
                "BOS descriptor length mismatch, buffer length: {}, total length: {}",
                bos.len(),
                total_length
            );
            return Vec::new();
        }
        let mut capability_descriptors = Vec::new();
        let mut data = &bos[5..];
        loop {
            if data.is_empty() {
                break;
            }
            let descriptor_length = data[0];
            if data.len() < descriptor_length as usize {
                error!(
                    "Invalid device capability descriptor inside BOS descriptor, rest buffer length: {}, descriptor length: {}",
                    data.len(),
                    descriptor_length
                );
                return Vec::new();
            }
            capability_descriptors.push(data[0..descriptor_length as usize].to_vec());
            data = &data[descriptor_length as usize..];
        }
        capability_descriptors
    }
    fn get_class_specific_descriptor(&self) -> Vec<u8> {
        Vec::new()
    }

    fn handle_urb(
        &mut self,
        _interface: &UsbInterface,
        _ep: UsbEndpoint,
        transfer_buffer_length: u32,
        setup: SetupPacket,
        req: &[u8],
    ) -> std::io::Result<Vec<u8>> {
        let control = ControlSetup::new(&setup, Some(req))?;
        match control {
            ControlSetup::In(control) if control.request == StandardRequest::GetStatus as u8 => {
                Ok(vec![0x00, 0x00])
            }
            ControlSetup::In(mut control) => {
                self.ccid
                    .lock()
                    .unwrap()
                    .as_any()
                    .downcast_mut::<CCIDInterfaceHandler>()
                    .unwrap()
                    .drop_card();
                if control.recipient == transfer::Recipient::Interface {
                    control.index &= 0xFF00;
                    control.index |= self.interface_number as u16;
                }
                let mut data = self
                    .interface
                    .control_in(control, Duration::from_secs(5))
                    .wait()
                    .map_err(io::Error::from)?;
                if data.len() > transfer_buffer_length as usize {
                    data.truncate(transfer_buffer_length as usize);
                }
                Ok(data)
            }
            ControlSetup::Out(mut control) => {
                self.ccid
                    .lock()
                    .unwrap()
                    .as_any()
                    .downcast_mut::<CCIDInterfaceHandler>()
                    .unwrap()
                    .drop_card();
                if control.recipient == transfer::Recipient::Interface {
                    control.index &= 0xFF00;
                    control.index |= self.interface_number as u16;
                }
                debug!(
                    "Out transfer control: {:02X?}, req: {:02X?}",
                    control_string(&ControlSetup::Out(control)),
                    req
                );
                self.interface
                    .control_out(control, Duration::from_secs(5))
                    .wait()
                    .map_err(io::Error::from)?;
                Ok(vec![])
            }
        }
    }

    fn as_any(&mut self) -> &mut dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use crate::device::ControlSetup;
    use crate::webusb::control_string;
    use log::{debug, error};
    use nusb::MaybeFuture;
    use nusb::transfer::{ControlIn, ControlOut, ControlType, Recipient};
    use std::io;
    use std::time::Duration;
    use usbip::DescriptorType;

    #[derive(Debug, Copy, Clone, PartialOrd, PartialEq)]
    enum TransferStatus {
        STATE_IDLE,
        STATE_PROCESS,
        STATE_SENDING_RESP,
        STATE_SENT_RESP,
    }

    impl Into<u8> for TransferStatus {
        fn into(self) -> u8 {
            match self {
                Self::STATE_IDLE => 0xff,
                Self::STATE_PROCESS => 0x01,
                Self::STATE_SENDING_RESP => 0x00,
                Self::STATE_SENT_RESP => 0x03,
            }
        }
    }

    impl TryFrom<u8> for TransferStatus {
        type Error = io::Error;
        fn try_from(value: u8) -> Result<Self, Self::Error> {
            Ok(match value {
                0xff => Self::STATE_IDLE,
                0x01 => Self::STATE_PROCESS,
                0x00 => Self::STATE_SENDING_RESP,
                0x03 => Self::STATE_SENT_RESP,
                other => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("Unknown TransferStatus value 0x{:02X}", other),
                    ));
                }
            })
        }
    }

    fn send_apdu(interface: &nusb::Interface, data: &[u8]) -> io::Result<()> {
        let control = ControlOut {
            control_type: ControlType::Vendor,
            recipient: Recipient::Interface,
            request: 0x0,
            value: 0x0,
            index: interface.interface_number() as u16,
            data,
        };
        debug!(
            "Out transfer OUT control: {:02X?}, data: {:02X?}",
            control_string(&ControlSetup::Out(control)),
            data
        );
        interface
            .control_out(control, Duration::from_secs(5))
            .wait()
            .map(|_| ())
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    fn received_apdu(interface: &nusb::Interface) -> io::Result<Vec<u8>> {
        loop {
            match current_transfer_state(interface)? {
                TransferStatus::STATE_PROCESS
                | TransferStatus::STATE_IDLE
                | TransferStatus::STATE_SENT_RESP => (),
                TransferStatus::STATE_SENDING_RESP => break,
            }
        }
        let control = ControlIn {
            control_type: ControlType::Vendor,
            recipient: Recipient::Interface,
            request: 0x1,
            value: 0x0,
            index: interface.interface_number() as u16,
            length: 4096,
        };
        let other = control.clone();
        let data = interface
            .control_in(control, Duration::from_secs(5))
            .wait()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        debug!(
            "Out transfer IN control: {:02X?}, data: {:02X?}",
            control_string(&ControlSetup::In(other)),
            data
        );
        Ok(data)
    }

    fn current_transfer_state(interface: &nusb::Interface) -> io::Result<TransferStatus> {
        let data = interface
            .control_in(
                ControlIn {
                    control_type: ControlType::Vendor,
                    recipient: Recipient::Interface,
                    request: 0x02, // WEBUSB_REQ_STAT
                    value: 0x00,
                    index: interface.interface_number() as u16,
                    length: 0x1,
                },
                Duration::from_secs(5),
            )
            .wait()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        if data.len() != 1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Invalid TransferStatus value size {}", data.len()),
            ));
        }
        TransferStatus::try_from(data[0])
    }

    #[test]
    fn test_ccid_claim() {
        let device = nusb::list_devices()
            .wait()
            .unwrap()
            .find(|dev| dev.vendor_id() == 0x20A0 && dev.product_id() == 0x42D4)
            .unwrap();
        let handle = device.open().wait().unwrap();
        let ccid = handle
            .active_configuration()
            .unwrap()
            .interfaces()
            .find(|interface| {
                interface
                    .alt_settings()
                    .find(|settings| settings.class() == 0x0B)
                    .is_some()
            })
            .unwrap();
        let _interface = handle
            .claim_interface(ccid.interface_number())
            .wait()
            .expect_err("Should fail");
    }

    #[test]
    fn test_libusb() {
        env_logger::init();
        let device = nusb::list_devices()
            .wait()
            .unwrap()
            .find(|dev| dev.vendor_id() == 0x20A0 && dev.product_id() == 0x42D4)
            .unwrap();
        let handle = device.open().wait().unwrap();
        let webusb = handle
            .active_configuration()
            .unwrap()
            .interfaces()
            .find(|interface| {
                interface
                    .alt_settings()
                    .find(|settings| settings.class() == 0xFF)
                    .is_some()
            })
            .unwrap();
        let interface = handle
            .claim_interface(webusb.interface_number())
            .wait()
            .unwrap();
        // let size = interface.control_out_blocking(Control {
        //     control_type: ControlType::Vendor,
        //     recipient: Recipient::Interface,
        //     request: 0x00, // WebUSB vendor
        //     value: 0x00, // WEBUSB_REQ_STAT
        //     index: interface.interface_number() as u16
        // },
        // &mut data,
        // Duration::from_millis(5000)).unwrap();
        // data.truncate(size);
        // println!("GET_URL result:\n{:#?}", data);
        //println!("REQUEST_URL result:\n{:#?}", str::from_utf8(&data).unwrap());
        // let result = received_apdu(&interface).unwrap();
        // error!("APDU result: {:02X?}", result);
        //error!("TransferStatus: {:?}", current_transfer_state(&interface).unwrap());
        send_apdu(
            &interface,
            &[0x00u8, 0xA4, 0x04, 0x00, 0x05, 0xF0, 0x00, 0x00, 0x00, 0x00],
        )
        .unwrap();
        //send_apdu(&interface, &[]).unwrap();
        // let mut status = current_transfer_state(&interface).unwrap();
        // error!("TransferStatus: {:?}", status);
        // while status == TransferStatus::STATE_PROCESS {
        //     status = current_transfer_state(&interface).unwrap();
        //     error!("TransferStatus: {:?}", status);
        // }
        //let mut result = received_apdu(&interface).unwrap();
        //error!("APDU result: {:02X?}", result);
        //send_apdu(&interface, &[0x00, 0x41, 0x00, 0x00, 0x02]).unwrap();
        let result = received_apdu(&interface).unwrap();
        error!("APDU result: {:02X?}", result);
        // let bos = handle.get_descriptor(DescriptorType::BOS as u8, 0, 0, Duration::from_secs(5)).unwrap();
        // //let device_capabilities = handle.get_descriptor(0x10, 0, 0, Duration::from_secs(5)).unwrap();
        // println!("BOS: {:02X?}", bos);
        // //println!("DeviceCapabilities: {:02X?}", device_capabilities);
        // let mut ms_20_desc = vec![0u8; 4096];
        // let size = interface.control_in_blocking(Control {
        //     control_type: ControlType::Vendor,
        //     recipient: Recipient::Device,
        //     request: 0x02,
        //     value: 0x0,
        //     index: 0x7
        // }, &mut ms_20_desc, Duration::from_secs(5)).unwrap();
        // ms_20_desc.truncate(size as usize);
        // println!("MS_20_DESC: {:02X?}", ms_20_desc);
        let conf_desc = handle
            .get_descriptor(
                DescriptorType::Configuration as u8,
                0x0,
                0x0,
                Duration::from_secs(5),
            )
            .wait()
            .unwrap();
        println!("Configuration Descriptor:\n{:02X?}", conf_desc);
    }
}
