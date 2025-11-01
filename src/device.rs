use log::{debug, error};
use nusb::transfer;
use nusb::transfer::{ControlIn, ControlOut, ControlType, Recipient};
use std::any::Any;
use std::cell::OnceCell;
use std::fmt::{Debug, Formatter};
use std::io;
use std::sync::{Arc, Mutex};
use usbip::{DescriptorType, SetupPacket, StandardRequest, UsbDeviceHandler, UsbInterfaceHandler};

pub struct CanokeyVirtDeviceHandler {
    vendor_handlers: Vec<Arc<Mutex<Box<dyn UsbInterfaceHandler + Send>>>>,
    bos_descriptors: OnceCell<Vec<u8>>,
}

impl Debug for CanokeyVirtDeviceHandler {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "CanokeyVirtDeviceHandler")
    }
}

#[derive(Debug)]
pub(crate) enum ControlSetup<'a> {
    In(nusb::transfer::ControlIn),
    Out(nusb::transfer::ControlOut<'a>),
}

#[allow(dead_code)]
impl<'a> ControlSetup<'a> {
    pub fn control_type(&self) -> ControlType {
        match self {
            ControlSetup::In(control) => control.control_type,
            ControlSetup::Out(control) => control.control_type,
        }
    }

    pub fn recipient(&self) -> Recipient {
        match self {
            ControlSetup::In(control) => control.recipient,
            ControlSetup::Out(control) => control.recipient,
        }
    }

    pub fn request(&self) -> u8 {
        match self {
            ControlSetup::In(control) => control.request,
            ControlSetup::Out(control) => control.request,
        }
    }

    pub fn value(&self) -> u16 {
        match self {
            ControlSetup::In(control) => control.value,
            ControlSetup::Out(control) => control.value,
        }
    }

    pub fn index(&self) -> u16 {
        match self {
            ControlSetup::In(control) => control.index,
            ControlSetup::Out(control) => control.index,
        }
    }

    pub fn new(setup: &'a SetupPacket, data: Option<&'a [u8]>) -> io::Result<ControlSetup<'a>> {
        let direction = match (setup.request_type & 0x80) != 0 {
            true => transfer::Direction::In,
            false => transfer::Direction::Out,
        };
        let control_type = match (setup.request_type & 0x60) >> 5 {
            0 => transfer::ControlType::Standard,
            1 => transfer::ControlType::Class,
            2 => transfer::ControlType::Vendor,
            other => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("Unknown USB setup packet control type {}", other),
                ));
            }
        };

        let recipient = match setup.request_type & 0x1F {
            0 => transfer::Recipient::Device,
            1 => transfer::Recipient::Interface,
            2 => transfer::Recipient::Endpoint,
            3 => transfer::Recipient::Other,
            other => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("Unknown USB setup packet recipient type {}", other),
                ));
            }
        };

        Ok(match direction {
            transfer::Direction::In => ControlSetup::In(ControlIn {
                control_type,
                recipient,
                request: setup.request,
                value: setup.value,
                index: setup.index,
                length: setup.length,
            }),
            transfer::Direction::Out => ControlSetup::Out(ControlOut {
                control_type,
                recipient,
                request: setup.request,
                value: setup.value,
                index: setup.index,
                data: data.unwrap_or_default(),
            }),
        })
    }
}

impl CanokeyVirtDeviceHandler {
    pub fn new(handlers: &[Arc<Mutex<Box<dyn UsbInterfaceHandler + Send>>>]) -> Self {
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
        let control = ControlSetup::new(&setup, Some(req))?;
        if control.control_type() == ControlType::Vendor {
            for handler in self.vendor_handlers.iter_mut() {
                match handler
                    .lock()
                    .unwrap()
                    .handle_device_urb(transfer_buffer_length, setup, req)
                {
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
        const GET_STATUS: u8 = StandardRequest::GetStatus as u8;
        const GET_DESCRIPTOR: u8 = StandardRequest::GetDescriptor as u8;
        match control {
            ControlSetup::In(control)
                if control.control_type == ControlType::Standard
                    && control.request == GET_STATUS =>
            {
                Ok(vec![0x00, 0x00])
            }
            ControlSetup::In(control)
                if control.control_type == ControlType::Standard
                    && control.recipient == Recipient::Device
                    && control.request == GET_DESCRIPTOR
                    && (((control.value & 0xFF00) >> 8) as u8) == DescriptorType::BOS as u8 =>
            {
                Ok(self.bos_descriptors.get_or_init(|| {
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
                        let total_length = capability_descriptors.iter().fold(5usize, |v, d| { v + d.len() });
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
                        capability_descriptors.into_iter().for_each(|v| bos_descriptors.extend(v));
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
