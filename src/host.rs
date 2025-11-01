//! Host USB
use super::*;
use nusb::MaybeFuture;

/// A handler to pass requests to interface of a rusb USB device of the host
#[derive(Clone, Debug)]
pub struct RusbUsbHostInterfaceHandler {
    handle: Arc<Mutex<DeviceHandle<GlobalContext>>>,
}

impl RusbUsbHostInterfaceHandler {
    pub fn new(handle: Arc<Mutex<DeviceHandle<GlobalContext>>>) -> Self {
        Self { handle }
    }
}

impl UsbInterfaceHandler for RusbUsbHostInterfaceHandler {
    fn handle_urb(
        &mut self,
        _interface: &UsbInterface,
        ep: UsbEndpoint,
        transfer_buffer_length: u32,
        setup: SetupPacket,
        req: &[u8],
    ) -> Result<Vec<u8>> {
        debug!("To host device: ep={ep:?} setup={setup:?} req={req:?}",);
        let mut buffer = vec![0u8; transfer_buffer_length as usize];
        let timeout = std::time::Duration::new(1, 0);
        let handle = self.handle.lock().unwrap();
        if ep.attributes == EndpointAttributes::Control as u8 {
            // control
            if let Direction::In = ep.direction() {
                // control in
                if let Ok(len) = handle.read_control(
                    setup.request_type,
                    setup.request,
                    setup.value,
                    setup.index,
                    &mut buffer,
                    timeout,
                ) {
                    return Ok(Vec::from(&buffer[..len]));
                }
            } else {
                // control out
                handle
                    .write_control(
                        setup.request_type,
                        setup.request,
                        setup.value,
                        setup.index,
                        req,
                        timeout,
                    )
                    .ok();
            }
        } else if ep.attributes == EndpointAttributes::Interrupt as u8 {
            // interrupt
            if let Direction::In = ep.direction() {
                // interrupt in
                if let Ok(len) = handle.read_interrupt(ep.address, &mut buffer, timeout) {
                    info!("intr in {:?}", &buffer[..len]);
                    return Ok(Vec::from(&buffer[..len]));
                }
            } else {
                // interrupt out
                handle.write_interrupt(ep.address, req, timeout).ok();
            }
        } else if ep.attributes == EndpointAttributes::Bulk as u8 {
            // bulk
            if let Direction::In = ep.direction() {
                // bulk in
                if let Ok(len) = handle.read_bulk(ep.address, &mut buffer, timeout) {
                    return Ok(Vec::from(&buffer[..len]));
                }
            } else {
                // bulk out
                handle.write_bulk(ep.address, req, timeout).ok();
            }
        }
        Ok(vec![])
    }

    fn get_class_specific_descriptor(&self) -> Vec<u8> {
        vec![]
    }

    fn as_any(&mut self) -> &mut dyn Any {
        self
    }
}

/// A handler to pass requests to device of a rusb USB device of the host
#[derive(Clone, Debug)]
pub struct RusbUsbHostDeviceHandler {
    handle: Arc<Mutex<DeviceHandle<GlobalContext>>>,
}

impl RusbUsbHostDeviceHandler {
    pub fn new(handle: Arc<Mutex<DeviceHandle<GlobalContext>>>) -> Self {
        Self { handle }
    }
}

impl UsbDeviceHandler for RusbUsbHostDeviceHandler {
    fn handle_urb(
        &mut self,
        transfer_buffer_length: u32,
        setup: SetupPacket,
        req: &[u8],
    ) -> Result<Vec<u8>> {
        debug!("To host device: setup={setup:?} req={req:?}");
        let mut buffer = vec![0u8; transfer_buffer_length as usize];
        let timeout = std::time::Duration::new(1, 0);
        let handle = self.handle.lock().unwrap();
        // control
        if setup.request_type & 0x80 == 0 {
            // control out
            handle
                .write_control(
                    setup.request_type,
                    setup.request,
                    setup.value,
                    setup.index,
                    req,
                    timeout,
                )
                .ok();
        } else {
            // control in
            if let Ok(len) = handle.read_control(
                setup.request_type,
                setup.request,
                setup.value,
                setup.index,
                &mut buffer,
                timeout,
            ) {
                return Ok(Vec::from(&buffer[..len]));
            }
        }
        Ok(vec![])
    }

    fn as_any(&mut self) -> &mut dyn Any {
        self
    }
}

/// A handler to pass requests to interface of a nusb USB device of the host
#[derive(Clone)]
pub struct NusbUsbHostInterfaceHandler {
    handle: Arc<Mutex<nusb::Interface>>,
}

impl std::fmt::Debug for NusbUsbHostInterfaceHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NusbUsbHostInterfaceHandler")
            .field("handle", &"Opaque")
            .finish()
    }
}

impl NusbUsbHostInterfaceHandler {
    pub fn new(handle: Arc<Mutex<nusb::Interface>>) -> Self {
        Self { handle }
    }
}

impl UsbInterfaceHandler for NusbUsbHostInterfaceHandler {
    fn handle_urb(
        &mut self,
        _interface: &UsbInterface,
        ep: UsbEndpoint,
        transfer_buffer_length: u32,
        setup: SetupPacket,
        req: &[u8],
    ) -> Result<Vec<u8>> {
        debug!("To host device: ep={ep:?} setup={setup:?} req={req:?}",);
        //let mut buffer = vec![0u8; transfer_buffer_length as usize];
        let timeout = std::time::Duration::new(1, 0);
        let handle = self.handle.lock().unwrap();
        if ep.attributes == EndpointAttributes::Control as u8 {
            // control
            if let Direction::In = ep.direction() {
                // control in
                let control = nusb::transfer::ControlIn {
                    control_type: match (setup.request_type >> 5) & 0b11 {
                        0 => nusb::transfer::ControlType::Standard,
                        1 => nusb::transfer::ControlType::Class,
                        2 => nusb::transfer::ControlType::Vendor,
                        _ => unimplemented!(),
                    },
                    recipient: match setup.request_type & 0b11111 {
                        0 => nusb::transfer::Recipient::Device,
                        1 => nusb::transfer::Recipient::Interface,
                        2 => nusb::transfer::Recipient::Endpoint,
                        3 => nusb::transfer::Recipient::Other,
                        _ => unimplemented!(),
                    },
                    request: setup.request,
                    value: setup.value,
                    index: setup.index,
                    length: transfer_buffer_length as u16,
                };
                if let Ok(data) = handle.control_in(control, timeout).wait() {
                    return Ok(data);
                }
            } else {
                // control out
                let control = nusb::transfer::ControlOut {
                    control_type: match (setup.request_type >> 5) & 0b11 {
                        0 => nusb::transfer::ControlType::Standard,
                        1 => nusb::transfer::ControlType::Class,
                        2 => nusb::transfer::ControlType::Vendor,
                        _ => unimplemented!(),
                    },
                    recipient: match setup.request_type & 0b11111 {
                        0 => nusb::transfer::Recipient::Device,
                        1 => nusb::transfer::Recipient::Interface,
                        2 => nusb::transfer::Recipient::Endpoint,
                        3 => nusb::transfer::Recipient::Other,
                        _ => unimplemented!(),
                    },
                    request: setup.request,
                    value: setup.value,
                    index: setup.index,
                    data: req,
                };
                handle.control_out(control, timeout).wait().ok();
            }
        } else if ep.attributes == EndpointAttributes::Interrupt as u8 {
            // interrupt
            todo!("Missing blocking api for interrupt transfer in nusb")
        } else if ep.attributes == EndpointAttributes::Bulk as u8 {
            // bulk
            todo!("Missing blocking api for bulk transfer in nusb")
        }
        Ok(vec![])
    }

    fn get_class_specific_descriptor(&self) -> Vec<u8> {
        vec![]
    }

    fn as_any(&mut self) -> &mut dyn Any {
        self
    }
}

/// A handler to pass requests to device of a nusb USB device of the host
#[derive(Clone)]
pub struct NusbUsbHostDeviceHandler {
    #[allow(dead_code)]
    handle: Arc<Mutex<nusb::Device>>,
}

impl std::fmt::Debug for NusbUsbHostDeviceHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NusbUsbHostDeviceHandler")
            .field("handle", &"Opaque")
            .finish()
    }
}

impl NusbUsbHostDeviceHandler {
    pub fn new(handle: Arc<Mutex<nusb::Device>>) -> Self {
        Self { handle }
    }
}

#[allow(unused_variables)]
impl UsbDeviceHandler for NusbUsbHostDeviceHandler {
    fn handle_urb(
        &mut self,
        transfer_buffer_length: u32,
        setup: SetupPacket,
        req: &[u8],
    ) -> Result<Vec<u8>> {
        debug!("To host device: setup={setup:?} req={req:?}");
        // control
        #[cfg(not(target_os = "windows"))]
        {
            let timeout = std::time::Duration::new(1, 0);
            let handle = self.handle.lock().unwrap();
            if setup.request_type & 0x80 == 0 {
                // control out
                let control = nusb::transfer::ControlOut {
                    control_type: match (setup.request_type >> 5) & 0b11 {
                        0 => nusb::transfer::ControlType::Standard,
                        1 => nusb::transfer::ControlType::Class,
                        2 => nusb::transfer::ControlType::Vendor,
                        _ => unimplemented!(),
                    },
                    recipient: match setup.request_type & 0b11111 {
                        0 => nusb::transfer::Recipient::Device,
                        1 => nusb::transfer::Recipient::Interface,
                        2 => nusb::transfer::Recipient::Endpoint,
                        3 => nusb::transfer::Recipient::Other,
                        _ => unimplemented!(),
                    },
                    request: setup.request,
                    value: setup.value,
                    index: setup.index,
                    data: req,
                };
                #[cfg(not(target_os = "windows"))]
                handle.control_out_blocking(control, req, timeout).ok();
            } else {
                // control in
                let control = nusb::transfer::ControlIn {
                    control_type: match (setup.request_type >> 5) & 0b11 {
                        0 => nusb::transfer::ControlType::Standard,
                        1 => nusb::transfer::ControlType::Class,
                        2 => nusb::transfer::ControlType::Vendor,
                        _ => unimplemented!(),
                    },
                    recipient: match setup.request_type & 0b11111 {
                        0 => nusb::transfer::Recipient::Device,
                        1 => nusb::transfer::Recipient::Interface,
                        2 => nusb::transfer::Recipient::Endpoint,
                        3 => nusb::transfer::Recipient::Other,
                        _ => unimplemented!(),
                    },
                    request: setup.request,
                    value: setup.value,
                    index: setup.index,
                    length: transfer_buffer_length as u16,
                };
                #[cfg(not(target_os = "windows"))]
                if let Ok(len) = handle.control_in_blocking(control, &mut buffer, timeout) {
                    return Ok(Vec::from(&buffer[..len]));
                }
            }
        }
        #[cfg(target_os = "windows")]
        {
            warn!("Not supported in windows")
        }
        Ok(vec![])
    }

    fn as_any(&mut self) -> &mut dyn Any {
        self
    }
}
