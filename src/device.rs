use log::{debug, error};
use nusb::transfer::{Control, ControlType, Direction, Recipient};
use std::any::Any;
use std::cell::OnceCell;
use std::fmt::{Debug, Formatter};
use std::io;
use std::sync::{Arc, Mutex};
use usbip::{DescriptorType, SetupPacket, StandardRequest, UsbDeviceHandler, UsbInterfaceHandler};

pub trait VendorControl: UsbInterfaceHandler + Send {
    fn handle_device_urb(
        &mut self,
        transfer_buffer_length: u32,
        setup: SetupPacket,
        req: &[u8],
    ) -> io::Result<Vec<u8>>;

    fn get_device_capability_descriptors(&self) -> Vec<Vec<u8>>;
}

pub struct CanokeyVirtDeviceHandler {
    vendor_handlers: Vec<Arc<Mutex<Box<dyn VendorControl>>>>,
    bos_descriptors: OnceCell<Vec<u8>>,
}

impl Debug for CanokeyVirtDeviceHandler {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "CanokeyVirtDeviceHandler")
    }
}

pub fn parseSetupPacket(
    setup: &SetupPacket,
) -> io::Result<(nusb::transfer::Direction, nusb::transfer::Control)> {
    let direction = match (setup.request_type & 0x80) != 0 {
        true => nusb::transfer::Direction::In,
        false => nusb::transfer::Direction::Out,
    };
    let control_type = match (setup.request_type & 0x60) >> 5 {
        0 => ControlType::Standard,
        1 => ControlType::Class,
        2 => ControlType::Vendor,
        other => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("Unknown USB setup packet control type {}", other),
            ));
        }
    };

    let recipient = match setup.request_type & 0x1F {
        0 => Recipient::Device,
        1 => Recipient::Interface,
        2 => Recipient::Endpoint,
        3 => Recipient::Other,
        other => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("Unknown USB setup packet recipient type {}", other),
            ));
        }
    };

    Ok((
        direction,
        Control {
            control_type,
            recipient,
            request: setup.request,
            value: setup.value,
            index: setup.index,
        },
    ))
}

impl CanokeyVirtDeviceHandler {
    pub fn new(handlers: &[Arc<Mutex<Box<dyn VendorControl>>>]) -> Self {
        Self {
            vendor_handlers: handlers.to_vec(),
            bos_descriptors: OnceCell::new(),
        }
    }
}

impl UsbDeviceHandler for CanokeyVirtDeviceHandler {
    fn handle_urb(
        &mut self,
        transfer_buffer_length: u32,
        setup: SetupPacket,
        req: &[u8],
    ) -> io::Result<Vec<u8>> {
        let request = (|v: (Direction, Control)| {
            let (direction, control) = v;
            (
                direction,
                control.control_type,
                control.recipient,
                control.request,
                control.value,
                control.index,
            )
        })(parseSetupPacket(&setup)?);
        match request {
            (_, ControlType::Vendor, _, _, _, _) => {
                for handler in self.vendor_handlers.iter_mut() {
                    match handler.lock().unwrap().handle_device_urb(
                        transfer_buffer_length,
                        setup,
                        req,
                    ) {
                        Ok(v) => return Ok(v),
                        Err(e) => {
                            error!(
                                "Current vendor handler failed to handle this vendor setup packet, try next: {}",
                                e
                            );
                        }
                    }
                }
            }
            _ => (),
        }
        const GET_STATUS: u8 = StandardRequest::GetStatus as u8;
        const GET_DESCRIPTOR: u8 = StandardRequest::GetDescriptor as u8;
        match request {
            (Direction::In, ControlType::Standard, _, GET_STATUS, _, _) => Ok(vec![0x00, 0x00]),
            (Direction::In, ControlType::Standard, Recipient::Device, GET_DESCRIPTOR, value, _)
                if (((value & 0xFF00) >> 8) as u8) == DescriptorType::BOS as u8 =>
            {
                Ok(self.bos_descriptors.get_or_init(||{
                    let default_bos_descriptor = vec![
                        0x05, // bLength
                        DescriptorType::BOS as u8, // bDescriptorType
                        0x05, 0x00, // wTotalLength
                        0x00, // bNumDeviceCaps
                    ];
                    let mut capability_descriptors = Vec::new();
                    for handler in self.vendor_handlers.iter() {
                        capability_descriptors.extend(handler.lock().unwrap().get_device_capability_descriptors());
                    }
                    let total_length = capability_descriptors.iter().fold(5usize, |v, d|{ v + d.len() });
                    let mut bos_descriptors = Vec::with_capacity(total_length);
                    if total_length > u16::MAX as usize {
                        error!("BOS descriptor is too long, total_length = {}, fallback to default", total_length);
                        return default_bos_descriptor;
                    }
                    if capability_descriptors.len() > u8::MAX as usize {
                        error!("Device capability descriptors exceeded limit, len = {}, fallback to default", capability_descriptors.len());
                        return default_bos_descriptor;
                    }
                    let total_length = total_length as u16;
                    bos_descriptors.extend_from_slice(&[0x05, DescriptorType::BOS as u8]);
                    bos_descriptors.extend(total_length.to_le_bytes());
                    bos_descriptors.push(capability_descriptors.len() as u8);
                    capability_descriptors.into_iter().for_each(|v|bos_descriptors.extend(v));
                    debug!("On init Device capability descriptors {:02X?}", bos_descriptors);
                    bos_descriptors
                }).clone())
            }
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Unknown setup request for device: {:02X?}", setup),
            )),
        }
    }

    fn as_any(&mut self) -> &mut dyn Any {
        self
    }
}
