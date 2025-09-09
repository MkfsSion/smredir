use std::any::Any;
use std::io;
use usbip::{SetupPacket, UsbEndpoint, UsbInterface, UsbInterfaceHandler};

#[derive(Debug)]
pub struct ReservedInterfaceHandler {}

impl ReservedInterfaceHandler {
    pub fn new() -> ReservedInterfaceHandler {
        Self {}
    }
}

impl UsbInterfaceHandler for ReservedInterfaceHandler {
    fn get_class_specific_descriptor(&self) -> Vec<u8> {
        vec![]
    }

    fn handle_urb(
        &mut self,
        interface: &UsbInterface,
        ep: UsbEndpoint,
        transfer_buffer_length: u32,
        setup: SetupPacket,
        req: &[u8],
    ) -> std::io::Result<Vec<u8>> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Attempt to access reserved USB interface",
        ))
    }

    fn as_any(&mut self) -> &mut dyn Any {
        self
    }
}
