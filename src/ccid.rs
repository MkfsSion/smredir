use crate::ccid_proto::{
    CCIDError, Decode, Encode, ICCClockStatus, ICCProtocol, Response, ResponseMessageHeader,
    SlotErrorRegister, SlotStatusRegister,
};
use crate::{ccid_const, ccid_proto};
use log::{debug, error};
use pcsc::{Attribute, Disposition, Protocols, Scope, ShareMode};
use std::any::Any;
use std::cell::{Cell, SyncUnsafeCell};
use std::collections::VecDeque;
use std::ffi::{CStr, CString};
use std::fmt::{Debug, Formatter};
use std::io;
use usbip::{EndpointAttributes, SetupPacket, UsbEndpoint, UsbInterface, UsbInterfaceHandler};

pub struct CCIDInterfaceHandler {
    context: pcsc::Context,
    card: Cell<Option<pcsc::Card>>,
    ccid_descriptor: Vec<u8>,
    outQueue: VecDeque<Vec<u8>>,
    reader_name: CString,
    parameter: Option<Vec<u8>>, // ProtocolData
}

impl Debug for CCIDInterfaceHandler {
    fn fmt(&self, f: &mut Formatter) -> Result<(), std::fmt::Error> {
        write!(f, "CCIDInterfaceHandler")
    }
}

// #[derive(Error, Debug)]
// pub enum CCIDBackendError {
//     #[error("Failed to establish PCSC context, status = 0x{0:08X}")]
//     ContextError(u32),
//     #[error("Failed to connect to reader '{0}', status = 0x{1:08X}")]
//     ConnectError(String, u32),
//     #[error("Failed to retrieve USB descriptor: {0}")]
//     USBDescriptor(String),
//     // #[error("Failed to {0} '{1}': {2}")]
//     // SerializeError(SerializeOperation, &'static str, String),
//     // #[error("Unknown CCID command type 0x{0:02X}")]
//     // UnknownCommandType(u8),
//     // #[error("Unknown CCID response type 0x{0:02X}")]
//     // UnknownResponseType(u8),
//     // #[error("Unknown bPowerSelect id 0x{0:2X} in PC_to_RDR_IccPowerOn message")]
//     // UnknownICCVoltage(u8),
// }

impl CCIDInterfaceHandler {
    pub fn new(
        reader_name: &CStr,
        device: &nusb::Device,
    ) -> Result<CCIDInterfaceHandler, io::Error> {
        let mut ccid_descriptor = vec![
            0x36, // bLength
            0x21, // bDescriptorType ( 21h => CCID )
            0x10, 0x01, // bcdCCID ( v1.10 )
            0x00, // bMaxSlotIndex ( 0 since we are redirect single card ),
            0x07, // bVoltageSupport ( Not apply )
            0x02, 0x00, 0x00, 0x00, // dwProtocols ( Force T=1 )
            0x00, 0x00, 0x00, 0x00, // dwDefaultClock ( Not apply )
            0x00, 0x00, 0x00, 0x00, // dwMaximumClock ( Not apply )
            0x00, // bNumClockSupported ( Not apply )
            0x00, 0x00, 0x00, 0x00, // dwDataRate ( 4MHz )
            0x00, 0x00, 0x00, 0x00, // dwMaxDataRate ( 4MHz )
            0x00, // bNumDataRatesSupported ( Card managed )
            0xF6, 0xFF, 0x00, 0x00, // dwMaxIFSD (65526 bytes (Prevent chaining))
            0x00, 0x00, 0x00, 0x00, // dwSynchProtocols
            0x00, 0x00, 0x00, 0x00, // dwMechanical
            0xFE, 0x00, 0x04,
            0x00, // dwFeatures ( All byte 1 characteristics and Short and Extended APDU level exchange with CCID)
            0x00, 0x00, 0x01, 0x00, // dwMaxCCIDMessageLength (65536 byte (Prevent chaining))
            0xFF, // bClassGetResponse (  CCID echoes the class of the APDU )
            0xFF, // bClassEnvelope (  CCID echoes the class of the APDU )
            0x00, 0x00, // wLcdLayout ( No LCD display ),
            0x00, // bPINSupport ( No CCID PIN support )
            0x01, // bMaxCCIDBusySlots ( 1 since we are redirect single card )
        ];
        let desc = device
            .active_configuration()
            .map_err(|e| io::Error::other(format!("Failed to get active configuration: {}", e)))?
            .descriptors()
            .find(|d| {
                d.descriptor_type() == 0x21 && d.descriptor_len() == 0x36 // CCID
            })
            .ok_or(io::Error::new(
                io::ErrorKind::NotFound,
                "Specified USB device does not have CCID class descriptor",
            ))?;
        // dwDefaultClock & dwMaximumClock
        ccid_descriptor[10..10 + 8].copy_from_slice(&desc[10..10 + 8]);
        // dwDataRate & dwMaxDataRate
        ccid_descriptor[19..19 + 8].copy_from_slice(&desc[19..19 + 8]);
        debug!("CCID descriptors: {:02X?}", ccid_descriptor);
        let context = pcsc::Context::establish(Scope::User).map_err(|e| {
            io::Error::other(format!(
                "Failed to create PCSC context, status = '0x{:08X}'",
                e as u32
            ))
        })?;
        let card = context
            .connect(reader_name, ShareMode::Exclusive, Protocols::T1)
            .map_err(|e| {
                io::Error::other(format!(
                    "Failed to connect to reader '{}', status = '0x{:08X}'",
                    reader_name.to_string_lossy(),
                    e as u32
                ))
            })?;
        debug!("Created reader '{}'", reader_name.to_string_lossy());
        let atr = card
            .get_attribute_owned(Attribute::AtrString)
            .map_err(|e| {
                io::Error::other(format!(
                    "Failed to get ATR from reader '{}', status = {:08X}",
                    reader_name.to_string_lossy(),
                    e as u32
                ))
            })?;
        if atr.len() < 2 {
            return Err(io::Error::other(format!(
                "ATR read from reader '{}' is too short, expects at least 2 bytes, got {} bytes",
                reader_name.to_string_lossy(),
                atr.len()
            )));
        }

        let parameter = (|| {
            let direct_convention = match atr[0] {
                0x3B => true,
                0x3F => false,
                _ => {
                    debug!(
                        "TS of ATR of reader '{}' has unknown value 0x{:02X}",
                        reader_name.to_string_lossy(),
                        atr[0]
                    );
                    return None;
                }
            };
            if atr[1] & 0x10 == 0 {
                debug!(
                    "TA1 bytes does not exists in ATR of reader '{}'",
                    reader_name.to_string_lossy()
                );
                return None;
            }
            if atr[1] & 0x40 == 0 {
                debug!(
                    "TC1 bytes does not exists in ATR of reader '{}'",
                    reader_name.to_string_lossy()
                );
            }
            let ta1_offset = 2usize; // If ATR has TA1, it must follow T0 byte
            let tc1_offset = match (atr[1] & 0xF0) >> 4 {
                0x8 | 0xA => {
                    debug!(
                        "Only TD1 byte exists in ATR of reader '{}'",
                        reader_name.to_string_lossy()
                    );
                    return None;
                }
                0x0 | 0x2 => {
                    debug!(
                        "None of TA1, TC1, TD1  bytes exist in ATR of reader '{}'",
                        reader_name.to_string_lossy()
                    );
                    return None;
                }
                0x1 | 0x3 => {
                    debug!(
                        "Only TA1 byte exists in ATR of reader '{}'",
                        reader_name.to_string_lossy()
                    );
                    return None;
                }
                0x9 | 0xB => {
                    debug!(
                        "Only TA1 and TD1 byte exists in ATR of reader '{}'",
                        reader_name.to_string_lossy()
                    );
                    return None;
                }
                0x4 | 0x6 => {
                    debug!(
                        "Only TC1 byte exists in ATR of reader '{}'",
                        reader_name.to_string_lossy()
                    );
                    return None;
                }
                0xC | 0xE => {
                    debug!(
                        "Only TC1 and TD1 byte exists in ATR of reader '{}'",
                        reader_name.to_string_lossy()
                    );
                    return None;
                }
                0x5 | 0x7 => {
                    debug!(
                        "Only TA1 and TC1 byte exists in ATR of reader '{}'",
                        reader_name.to_string_lossy()
                    );
                    return None;
                }
                0xD => ta1_offset + 1,
                0xF => ta1_offset + 2,
                _ => unreachable!(),
            };
            let td1_offset = tc1_offset + 1;
            if ta1_offset >= atr.len() {
                debug!(
                    "ATR is too short to contain TA1 byte, TA1 offset = {}, length = {}",
                    ta1_offset,
                    atr.len()
                );
                return None;
            }
            if tc1_offset >= atr.len() {
                debug!(
                    "ATR is too short to contain TC1 byte, TC1 offset = {}, length = {}",
                    tc1_offset,
                    atr.len()
                );
                return None;
            }
            if td1_offset >= atr.len() {
                debug!(
                    "ATR is too short to contain TD1 byte, TD1 offset = {}, length = {}",
                    td1_offset,
                    atr.len()
                );
                return None;
            }
            let ta1 = atr[ta1_offset];
            let tc1 = atr[tc1_offset];
            let td1 = atr[td1_offset];
            // If T=1, lowest bit of first TC byte means if CRC is used
            // In the meantime, as per ISO-7816-3, TC1 also encodes Extra Guard Time
            let tcckst1 = match (tc1 & 0x01 == 0x01, !direct_convention) {
                (true, true) => 3u8,
                (true, false) => 1,
                (false, true) => 2,
                (false, false) => 0,
            } | 0x10;
            let extra_guard_time = tc1;
            let td2_offset = match (td1 & 0xF0) >> 4 {
                0x8 | 0xC => td1_offset + 1,
                0x9 | 0xA => td1_offset + 2,
                0xB | 0xD | 0xE => td1_offset + 3,
                0xF => td1_offset + 4,
                v => {
                    debug!(
                        "ATR of of reader '{}' does not contain TD2 byte, since Y1 is 0x{:X}",
                        reader_name.to_string_lossy(),
                        v
                    );
                    return None;
                }
            };
            if td2_offset >= atr.len() {
                debug!(
                    "ATR is too short to contain TD2 byte, TD2 offset = {}, length = {}",
                    td2_offset,
                    atr.len()
                );
                return None;
            }
            let td2 = atr[td2_offset];
            match (td2 & 0xF0) >> 4 {
                0x1 | 0x5 | 0x9 | 0xD => {
                    debug!(
                        "Only TA3 byte exists in ATR of reader '{}'",
                        reader_name.to_string_lossy()
                    );
                    return None;
                }
                0x2 | 0x6 | 0xA | 0xE => {
                    debug!(
                        "Only TB3 byte exists in ATR of reader '{}'",
                        reader_name.to_string_lossy()
                    );
                    return None;
                }
                0x3 | 0x7 | 0xB | 0xF => (),
                _ => {
                    debug!(
                        "Neither TA3 nor TB3 bytes exist in ATR of reader '{}'",
                        reader_name.to_string_lossy()
                    );
                }
            }
            if td2_offset + 2 >= atr.len() {
                debug!(
                    "ATR is too short to contain TA3 and TB3 bytes, TD3 offset = {}, length = {}",
                    td2_offset + 1,
                    atr.len()
                );
                return None;
            }
            let ta3 = atr[td2_offset + 1];
            let tb3 = atr[td2_offset + 2];

            Some(vec![
                ta1,
                tcckst1,
                extra_guard_time,
                tb3,
                0x00, //  Stopping the Clock is not allowed
                ta3,
                0x0, // NAD value
            ])
        })();

        if parameter.is_none() {
            debug!(
                "Failed to generate CCID parameters, will fail GetParameter request with unsupported command error"
            );
        }

        Ok(Self {
            context,
            card: Cell::new(Some(card)),
            ccid_descriptor,
            outQueue: VecDeque::new(),
            reader_name: reader_name.to_owned(),
            parameter,
        })
    }

    pub fn endpoints() -> Vec<UsbEndpoint> {
        vec![
            // Bulk IN device to host (response)
            UsbEndpoint {
                address: 0x81,
                attributes: EndpointAttributes::Bulk as u8,
                max_packet_size: 0x200,
                interval: 0,
            },
            // Bulk OUT host to device (command)
            UsbEndpoint {
                address: 0x01,
                attributes: EndpointAttributes::Bulk as u8,
                max_packet_size: 0x200,
                interval: 0,
            },
        ]
    }
}

impl CCIDInterfaceHandler {
    pub fn drop_card(&mut self) {
        if self.card.get_mut().is_some() {
            if let Err(e) = self.card.take().unwrap().disconnect(Disposition::ResetCard) {
                error!("Failed to disconnect reset card: {:?}", e.1);
            }
            debug!("PC_to_RDR_IccPowerOff: Disconnected reset card");
        }
    }
}

impl UsbInterfaceHandler for CCIDInterfaceHandler {
    fn get_class_specific_descriptor(&self) -> Vec<u8> {
        self.ccid_descriptor.clone()
    }

    fn handle_urb(
        &mut self,
        _interface: &UsbInterface,
        ep: UsbEndpoint,
        _transfer_buffer_length: u32,
        setup: SetupPacket,
        req: &[u8],
    ) -> io::Result<Vec<u8>> {
        if ep.is_ep0() {
            match setup.request {
                // Abort
                0x01 => {
                    debug!("CCID Setup ABORT request: {:?}", setup);
                }
                // GET_CLOCK_FREQUENCIES
                0x02 => {
                    debug!("CCID Setup GET_CLOCK_FREQUENCIES request: {:?}", setup);
                    //  Unsupported
                }
                // GET_DATA_RATES
                0x03 => {
                    debug!("CCID Setup GET_DATA_RATES request: {:?}", setup);
                    // Unsupported
                }
                _ => {
                    debug!("Unknown SETUP request: {:?}", setup);
                }
            }
            if setup.request != 0x01 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Invalid setup request",
                ));
            }
            Ok(vec![])
        } else {
            match ep.address | (setup.request_type & 0x80) {
                0x81 => {
                    debug!("CCID Bulk IN request: {:?}", setup);
                    match self.outQueue.pop_front() {
                        None => Ok(vec![]),
                        Some(v) => Ok(v),
                    }
                }
                0x01 => {
                    if req.len() < 10 {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidInput,
                            format!(
                                "Invalid transfer buffer length {}, CCID message must be at least 10 bytes long",
                                req.len()
                            ),
                        ));
                    }
                    let mut data = io::Cursor::new(req);
                    let cmd = match ccid_proto::Command::decode(&mut data) {
                        Ok(cmd) => cmd,
                        Err(CCIDError::BadCommand) => {
                            return Err(io::Error::new(
                                io::ErrorKind::InvalidData,
                                "Command bytes is not valid: CCIDError::BadCommand",
                            ));
                        }
                        Err(CCIDError::CommandError(header)) => {
                            error!("Failed to decode command: {:?}", header);
                            let mut data = io::Cursor::new(Vec::new());
                            ccid_proto::Response::new_with_error(header)
                                .encode(&mut data)
                                .unwrap();
                            self.outQueue.push_back(data.into_inner());
                            return Ok(vec![]);
                        }
                    };
                    error!("CCID command: {:02X?}", cmd);
                    let response;
                    if self.card.get_mut().is_none()
                        && cmd.get_header().bMessageType != ccid_const::PC_to_RDR_IccPowerOn
                        && cmd.get_header().bMessageType != ccid_const::PC_to_RDR_IccPowerOff
                        && cmd.get_header().bMessageType != ccid_const::PC_to_RDR_GetSlotStatus
                    {
                        debug!("Attempt to access disconnected card");
                        response =
                            ccid_proto::Response::new_with_error(ResponseMessageHeader::new(
                                *cmd.get_header(),
                                SlotStatusRegister::ICCAbsentFailure,
                                SlotErrorRegister::InvalidParameter(0x5),
                            ));
                        debug!("Response: {:02X?}", response);
                    } else if cmd.get_header().bSlot != 0x0 {
                        debug!(
                            "Attempt to access non-exists CCID slot {}",
                            cmd.get_header().bSlot
                        );
                        response =
                            ccid_proto::Response::new_with_error(ResponseMessageHeader::new(
                                *cmd.get_header(),
                                SlotStatusRegister::ICCAbsentFailure,
                                SlotErrorRegister::InvalidParameter(0x05),
                            ));
                    } else {
                        match cmd {
                            ccid_proto::Command::PC_to_RDR_Abort { header, .. } => {
                                response = ccid_proto::Response::new(header);
                            }
                            ccid_proto::Command::PC_to_RDR_GetSlotStatus { header, .. } => {
                                let mut resp = ccid_proto::Response::new(header);
                                if self.card.get_mut().is_none() {
                                    resp.set_status(
                                        SlotStatusRegister::ICCInactiveSuccess,
                                        SlotErrorRegister::UnsupportedCommand,
                                    );
                                }
                                response = resp;
                            }
                            ccid_proto::Command::PC_to_RDR_IccPowerOff { header, .. } => {
                                let mut resp = ccid_proto::Response::new(header);
                                if let ccid_proto::Response::RDR_to_PC_SlotStatus {
                                    header,
                                    bClockStatus,
                                } = &mut resp
                                {
                                    header.bStatus = SlotStatusRegister::ICCInactiveSuccess;
                                    header.bError = SlotErrorRegister::UnsupportedCommand;
                                    *bClockStatus = ICCClockStatus::Running;
                                }
                                self.drop_card();
                                response = resp;
                            }
                            ccid_proto::Command::PC_to_RDR_IccPowerOn { header, .. } => {
                                let mut resp = ccid_proto::Response::new(header);
                                (|| {
                                    if self.card.get_mut().is_none() {
                                        let card = match self.context.connect(
                                            &self.reader_name,
                                            ShareMode::Exclusive,
                                            Protocols::T1,
                                        ) {
                                            Ok(card) => card,
                                            Err(e) => {
                                                debug!("Failed to connect card: {:?}", e);
                                                resp.set_status(
                                                    SlotStatusRegister::ICCInactiveFailure,
                                                    SlotErrorRegister::HardwareError,
                                                );
                                                return;
                                            }
                                        };
                                        self.card.set(Some(card));
                                    }
                                    let status =
                                        match self.card.get_mut().as_ref().unwrap().status2_owned()
                                        {
                                            Ok(status) => status,
                                            Err(e) => {
                                                debug!("Failed to get card status: {:?}", e);
                                                resp.set_status(
                                                    SlotStatusRegister::ICCInactiveFailure,
                                                    SlotErrorRegister::HardwareError,
                                                );
                                                return;
                                            }
                                        };
                                    resp.append(status.atr()).unwrap();
                                })();
                                response = resp;
                            }
                            ccid_proto::Command::PC_to_RDR_XfrBlock { header, abData, .. } => {
                                let mut resp = ccid_proto::Response::new(header);
                                if header.dwLength > 0 {
                                    match self.card.get_mut().as_mut().unwrap().transaction() {
                                        Ok(tx) => {
                                            static responseData: SyncUnsafeCell<[u8; 65536]> =
                                                SyncUnsafeCell::new([0u8; 65536]);
                                            match tx.transmit(&abData, unsafe {
                                                &mut *responseData.get()
                                            }) {
                                                Ok(apdu) => {
                                                    resp.append(apdu).unwrap();
                                                }
                                                Err(e) => {
                                                    debug!("SCardTransmit failed: {}", e);
                                                    if let ccid_proto::Response::RDR_to_PC_DataBlock { header, bChainParameter: _, abData: _ } = &mut resp {
                                                        header.bError = SlotErrorRegister::CommandSlotBusy;
                                                        header.bStatus = SlotStatusRegister::ICCActiveFailure;
                                                    }
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            debug!("SCardBeginTransaction failed: {}", e);
                                            if let ccid_proto::Response::RDR_to_PC_DataBlock {
                                                header,
                                                bChainParameter: _,
                                                abData: _,
                                            } = &mut resp
                                            {
                                                header.bError = SlotErrorRegister::CommandSlotBusy;
                                                header.bStatus =
                                                    SlotStatusRegister::ICCActiveFailure;
                                            }
                                        }
                                    }
                                }
                                response = resp;
                            }
                            ccid_proto::Command::PC_to_RDR_GetParameters { header, .. } => {
                                let mut resp;
                                if self.parameter.is_some() {
                                    resp = ccid_proto::Response::new(header);
                                    match &mut resp {
                                        Response::RDR_to_PC_Parameters { bProtocolNum, .. } => {
                                            *bProtocolNum = ICCProtocol::T1;
                                        }
                                        other => panic!("Unexpected response type: {:?}", other),
                                    }
                                    resp.append(self.parameter.as_ref().unwrap()).unwrap();
                                } else {
                                    resp = ccid_proto::Response::new_with_error(
                                        ResponseMessageHeader::new(
                                            header,
                                            SlotStatusRegister::ICCActiveFailure,
                                            SlotErrorRegister::UnsupportedCommand,
                                        ),
                                    );
                                }
                                response = resp;
                            }
                            ccid_proto::Command::PC_to_RDR_Escape { header, .. }
                            | ccid_proto::Command::PC_to_RDR_IccClock { header, .. }
                            | ccid_proto::Command::PC_to_RDR_Mechanical { header, .. }
                            | ccid_proto::Command::PC_to_RDR_ResetParameters { header, .. }
                            | ccid_proto::Command::PC_to_RDR_Secure { header, .. }
                            | ccid_proto::Command::PC_to_RDR_SetDataRateAndClockFrequency {
                                header,
                                ..
                            }
                            | ccid_proto::Command::PC_to_RDR_SetParameters { header, .. }
                            | ccid_proto::Command::PC_to_RDR_T0APDU { header, .. } => {
                                response = ccid_proto::Response::new_with_error(
                                    ResponseMessageHeader::new(
                                        header,
                                        SlotStatusRegister::ICCActiveFailure,
                                        SlotErrorRegister::UnsupportedCommand,
                                    ),
                                );
                            }
                        }
                    }
                    let mut data = io::Cursor::new(Vec::new());
                    response.encode(&mut data).unwrap();
                    let data = data.into_inner();
                    self.outQueue.push_back(data.clone());
                    debug!("CCID response bytes: {:02X?}", data);
                    Ok(vec![])
                }
                other => {
                    debug!("Unexpected endpoint address: 0x{:02X}", other);
                    Ok(vec![])
                }
            }
        }
    }

    fn as_any(&mut self) -> &mut dyn Any {
        self
    }
}
