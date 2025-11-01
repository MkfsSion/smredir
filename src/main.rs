#![feature(sync_unsafe_cell)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]
#![allow(clippy::uninlined_format_args)]
#![allow(clippy::cloned_ref_to_slice_refs)]
#![allow(clippy::enum_variant_names)]
#![allow(clippy::upper_case_acronyms)]
extern crate core;
use nusb::MaybeFuture;

use crate::device::CanokeyVirtDeviceHandler;
use crate::fido::FIDOInterfaceHandler;
use crate::webusb::WebUSBInterfaceHandler;
use env_logger::Builder;
use log::LevelFilter;
use std::fs::File;
use std::io::Write;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex};
use usbip::{UsbDevice, UsbDeviceHandler, UsbInterfaceHandler, UsbIpServer, UsbSpeed};

mod ccid;
mod ccid_const;
mod ccid_proto;
mod device;
mod fido;
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
        .filter(None, LevelFilter::Off)
        .init();
    let usb_device = nusb::list_devices()
        .wait()
        .expect("list_devices failed")
        .find(|device| device.vendor_id() == 0x20A0 && device.product_id() == 0x42D4)
        .expect("Failed to find Canokey pigeon device")
        .open()
        .wait()
        .expect("Failed to open Canokey pigeon device");
    let ccid_handler = Arc::new(Mutex::new(Box::new(
        ccid::CCIDInterfaceHandler::new(c"canokeys.org OpenPGP PIV OATH 0", &usb_device).unwrap(),
    )
        as Box<dyn usbip::UsbInterfaceHandler + Send>));
    let webusb_handler = Arc::new(Mutex::new(Box::new(
        WebUSBInterfaceHandler::new(usb_device.clone(), 1)
            .expect("Failed to create WebUSB InterfaceHandler"),
    ) as Box<dyn UsbInterfaceHandler + Send>));

    let device_handler =
        Arc::new(Mutex::new(
            Box::new(CanokeyVirtDeviceHandler::new(&[webusb_handler.clone()]))
                as Box<dyn UsbDeviceHandler + Send>,
        ));
    let fido_handler = Arc::new(Mutex::new(Box::new(
        FIDOInterfaceHandler::new(usb_device.clone())
            .expect("Failed to create FIDO InterfaceHandler"),
    ) as Box<dyn UsbInterfaceHandler + Send>));
    let mut v = UsbDevice::new(0)
        .with_device_handler(device_handler)
        .with_interface_and_number(
            0x03,
            0x00,
            0x00,
            0x00,
            Some("FIDO/U2F"),
            FIDOInterfaceHandler::endpoints(),
            fido_handler,
        )
        .with_interface_and_number(
            0xFF,
            0xFF,
            0xFF,
            0x1,
            Some("WebUSB"),
            vec![],
            webusb_handler,
        )
        .with_interface_and_number(
            0x0B,
            0x00,
            0x00,
            0x02,
            Some("OpenPGP PIV OATH"),
            ccid::CCIDInterfaceHandler::endpoints(),
            ccid_handler,
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
