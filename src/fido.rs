use crate::device::ControlSetup;
use hidapi::MAX_REPORT_DESCRIPTOR_SIZE;
use log::debug;
use nusb::transfer::{ControlType, Recipient};
use std::any::Any;
use std::fmt::Debug;
use std::io;
use usbip::StandardRequest::GetDescriptor;
use usbip::hid::HidDescriptorType;
use usbip::{EndpointAttributes, SetupPacket, UsbEndpoint, UsbInterface, UsbInterfaceHandler};

#[derive(Debug)]
pub struct FIDOInterfaceHandler {
    class_desc: Vec<u8>,
    device: hidapi::HidDevice,
    report_desc: Option<Vec<u8>>,
}

impl FIDOInterfaceHandler {
    pub fn new(device: nusb::Device) -> io::Result<FIDOInterfaceHandler> {
        let desc = device.device_descriptor();
        let hidapi = hidapi::HidApi::new().map_err(|e| {
            io::Error::other(format!("Failed to initialize HID API library: {}", e))
        })?;

        let dev_info = hidapi
            .device_list()
            .find(|dev| {
                dev.vendor_id() == desc.vendor_id()
                    && desc.product_id() == desc.product_id()
                    && dev.usage_page() == 0xF1D0
            })
            .ok_or(io::Error::new(
                io::ErrorKind::NotFound,
                format!(
                    "No FIDO device with PID = 0x{:04X}, VID = {:04X} found",
                    desc.vendor_id(),
                    desc.product_id()
                ),
            ))?;
        let descs = device.active_configuration()?.interfaces().find(|intf| {
            intf.interface_number() == dev_info.interface_number() as u8
        }).ok_or(io::Error::new(io::ErrorKind::NotFound, format!("Failed to get interface descriptors of FIDO device with PID = 0x{:04X}, VID = {:04X}", desc.vendor_id(), desc.product_id())))?;
        let mut class_desc = None;
        for setting in descs.alt_settings() {
            class_desc = setting
                .descriptors()
                .find(|d| d.descriptor_type() == 0x21 && d.descriptor_len() == 0x09);
            if class_desc.is_some() {
                break;
            }
        }
        let class_desc = match class_desc {
            Some(desc) => desc,
            None => return Err(io::Error::new(io::ErrorKind::NotFound, format!("No HID class descriptor of FIDO device with PID = 0x{:04X}, VID = {:04X} found", desc.vendor_id(), desc.product_id())))
        }.to_vec();

        debug!("FIDO class desc: {:02X?}", class_desc);

        let device = dev_info.open_device(&hidapi).map_err(|e| {
            io::Error::other(format!(
                "Failed to open FIDO device with PID = 0x{:04X}, VID = {:04X}: {}",
                desc.vendor_id(),
                desc.product_id(),
                e
            ))
        })?;
        Ok(Self {
            class_desc,
            device,
            report_desc: None,
        })
    }

    pub fn endpoints() -> Vec<UsbEndpoint> {
        vec![
            UsbEndpoint {
                address: 0x82,
                attributes: EndpointAttributes::Interrupt as u8,
                max_packet_size: 64,
                interval: 6,
            },
            UsbEndpoint {
                address: 0x02,
                attributes: EndpointAttributes::Interrupt as u8,
                max_packet_size: 64,
                interval: 6,
            },
        ]
    }
}

impl UsbInterfaceHandler for FIDOInterfaceHandler {
    fn get_class_specific_descriptor(&self) -> Vec<u8> {
        debug!("FIDO: get_class_specific_descriptor");
        self.class_desc.clone()
    }
    fn handle_urb(
        &mut self,
        _interface: &UsbInterface,
        ep: UsbEndpoint,
        transfer_buffer_length: u32,
        setup: SetupPacket,
        req: &[u8],
    ) -> std::io::Result<Vec<u8>> {
        debug!(
            "FIDO: handle_urb: ep: {:0X?}, transfer_buffer_length: {:0X?}, setup: {:02X?}, req: {:02X?}",
            ep, transfer_buffer_length, setup, req
        );
        if ep.is_ep0() {
            let control = ControlSetup::new(&setup, Some(req))?;
            match control {
                ControlSetup::In(control)
                    if control.control_type == ControlType::Standard
                        && control.recipient == Recipient::Interface
                        && control.request == GetDescriptor as u8 =>
                {
                    match (control.value >> 8) as u8 {
                        v if v == HidDescriptorType::Report as u8 => {
                            if self.report_desc.is_none() {
                                let mut buffer = vec![0u8; MAX_REPORT_DESCRIPTOR_SIZE];
                                let size = self.device.get_report_descriptor(&mut buffer).map_err(
                                    |e| {
                                        io::Error::other(format!(
                                            "Failed to get HID report descriptor from device: {}",
                                            e
                                        ))
                                    },
                                )?;
                                buffer.truncate(size);
                                self.report_desc = Some(buffer);
                            }
                            let mut out = self.report_desc.clone().unwrap();
                            if out.len() > transfer_buffer_length as usize {
                                out.truncate(transfer_buffer_length as usize);
                            }
                            Ok(out)
                        }
                        v => Err(io::Error::other(format!(
                            "Unknown HID descriptor type of GET_DESCRIPTOR request: {:0X}",
                            v
                        ))),
                    }
                }
                ControlSetup::Out(control)
                    if control.control_type == ControlType::Class
                        && control.recipient == Recipient::Interface
                        && control.request == 0x0Au8 =>
                {
                    debug!("FIDO: Received SetIdle HID request: {:0X?}", control);
                    Ok(vec![])
                }
                other => Err(io::Error::other(format!(
                    "Unknown control request for FIDO HID interface: {:0X?}",
                    other
                ))),
            }
        } else {
            match ep.address {
                0x82 => {
                    // interrupt IN
                    let mut report = vec![0u8; transfer_buffer_length as usize];
                    match self.device.read_timeout(&mut report, 4) {
                        Ok(v) => {
                            debug!("FIDO Interrupt IN: Read {:0X?} bytes from device", v);
                            report.truncate(v);
                            Ok(report)
                        }
                        Err(e) => {
                            debug!(
                                "FIDO Interrupt IN: Unable to read HID report from device: {}",
                                e
                            );
                            Ok(Vec::new())
                        }
                    }
                }
                0x02 => {
                    let mut req = req.to_vec();
                    req.insert(0, 0x0);
                    match self.device.write(&req) {
                        Ok(v) => {
                            debug!("FIDO Interrupt OUT: Write {:0X?} bytes to device", v);
                            Ok(Vec::new())
                        }
                        Err(e) => Err(io::Error::other(format!(
                            "Failed to write HID request to device: {}",
                            e
                        ))),
                    }
                }
                _ => Err(io::Error::other(format!(
                    "Unknown endpoint address: {:0X?}",
                    ep.address
                ))),
            }
        }
    }

    fn as_any(&mut self) -> &mut dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use hidapi::MAX_REPORT_DESCRIPTOR_SIZE;

    #[test]
    fn test_hid() {
        let api = hidapi::HidApi::new().unwrap();
        let cnk = api
            .device_list()
            .find(|dev| {
                dev.vendor_id() == 0x20A0
                    && dev.product_id() == 0x42D4
                    && dev.usage_page() == 0xF1D0
            })
            .unwrap();
        let handle = cnk.open_device(&api).unwrap();
        let mut data = vec![0u8; MAX_REPORT_DESCRIPTOR_SIZE];
        let size = handle.get_report_descriptor(&mut data).unwrap();
        data.truncate(size);
        println!("Descriptor: {:#0X?}", data);
    }
}
