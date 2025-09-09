#![feature(sync_unsafe_cell)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]
extern crate core;

use crate::device::{CanokeyVirtDeviceHandler, VendorControl};
use crate::reserved::ReservedInterfaceHandler;
use crate::webusb::{WebUSBInterfaceHandler, WebUSBInterfaceInternalHandler};
use env_logger::Builder;
use log::LevelFilter;
use std::ffi::CStr;
use std::fs::File;
use std::io::Write;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex};
use usbip::{UsbDevice, UsbDeviceHandler, UsbInterfaceHandler, UsbIpServer, UsbSpeed};

mod ccid;
mod ccid_const;
mod ccid_proto;
mod device;
mod reserved;
mod webusb;

#[tokio::main]
async fn main() {
    //env_logger::init();
    let target = Box::new(File::create("smredir.log").expect("Can't create log file"));

    Builder::new()
        .format(|buf, record| {
            writeln!(
                buf,
                "{}:{} {} [{}] - {}",
                record.file().unwrap_or("unknown"),
                record.line().unwrap_or(0),
                chrono::Local::now().format("%Y-%m-%dT%H:%M:%S%.3f"),
                record.level(),
                record.args()
            )
        })
        .target(env_logger::Target::Pipe(target))
        .filter(None, LevelFilter::Trace)
        .init();
    let usb_device = nusb::list_devices()
        .expect("list_devices failed")
        .find(|device| device.vendor_id() == 0x20A0 && device.product_id() == 0x42D4)
        .expect("Failed to find Canokey pigeon device")
        .open()
        .expect("Failed to open Canokey pigeon device");
    let ccid_handler = Arc::new(Mutex::new(Box::new(
        ccid::CCIDInterfaceHandler::new(
            CStr::from_bytes_with_nul(b"canokeys.org OpenPGP PIV OATH 0\0").unwrap(),
            &usb_device,
        )
        .unwrap(),
    )
        as Box<dyn usbip::UsbInterfaceHandler + Send>));
    let webusb_handler = Arc::new(Mutex::new(Box::new(
        WebUSBInterfaceInternalHandler::new(usb_device.clone(), 1)
            .expect("Failed to create WebUSB InterfaceHandler"),
    ) as Box<dyn VendorControl>));

    let device_handler =
        Arc::new(Mutex::new(
            Box::new(CanokeyVirtDeviceHandler::new(&[webusb_handler.clone()]))
                as Box<dyn UsbDeviceHandler + Send>,
        ));
    let mut v = UsbDevice::new(0)
        .with_device_handler(device_handler)
        .with_interface_and_number(
            0xFF,
            0xFF,
            0xFF,
            0x00,
            Some("Reserved"),
            vec![],
            Arc::new(Mutex::new(Box::new(ReservedInterfaceHandler::new()))),
        )
        .with_interface_and_number(
            0xFF,
            0xFF,
            0xFF,
            0x1,
            Some("WebUSB"),
            vec![],
            Arc::new(Mutex::new(
                Box::new(WebUSBInterfaceHandler::new(webusb_handler))
                    as Box<dyn UsbInterfaceHandler + Send>,
            )),
        )
        .with_interface_and_number(
            0x0B,
            0x00,
            0x00,
            0x02,
            Some("OpenPGP PIV OATH"),
            ccid::CCIDInterfaceHandler::endpoints(),
            ccid_handler.clone(),
        );
    v.speed = UsbSpeed::High as u32;
    v.vendor_id = 0x20A0;
    v.product_id = 0x42D4;
    v.set_product_name("Canokey Relay Card").unwrap();
    v.set_manufacturer_name("canokeys.org").unwrap();
    v.set_serial_number("AAAABBBBCC").unwrap();
    v.unset_configuration_name().unwrap();
    v.usb_version.major = 0x2;
    v.usb_version.minor = 0x10;
    v.usb_version.patch = 0x0;
    v.device_bcd.major = 0x1;
    v.device_bcd.minor = 0x0;
    v.device_bcd.patch = 0x0;

    let server = Arc::new(UsbIpServer::new_simulated(vec![v]));

    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 3240);
    let _ = tokio::spawn(usbip::server(addr, server)).await;

    // loop {
    //     // sleep 1s
    //     tokio::time::sleep(Duration::new(1, 0)).await;
    // }
}
