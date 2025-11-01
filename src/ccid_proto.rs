use crate::ccid_const;
use byteorder::{LittleEndian, WriteBytesExt};
use log::debug;
use std::ops::{Deref, DerefMut};

#[derive(Debug, Copy, Clone)]
pub struct CommonMessageHeader {
    pub bMessageType: u8,
    pub dwLength: u32,
    pub bSlot: u8,
    pub bSeq: u8,
}

pub enum CCIDError {
    BadCommand,
    CommandError(ResponseMessageHeader),
}

impl CCIDError {
    pub fn command_error(
        command: CommonMessageHeader,
        status: SlotStatusRegister,
        error: SlotErrorRegister,
    ) -> CCIDError {
        CCIDError::CommandError(ResponseMessageHeader {
            inner: command,
            bStatus: status,
            bError: error,
        })
    }
}

impl Decode for CommonMessageHeader {
    type Error = CCIDError;
    fn decode<T: byteorder::ReadBytesExt>(input: &mut T) -> Result<Self, Self::Error> {
        let bMessageType = input.read_u8().map_err(|_| CCIDError::BadCommand)?;
        let dwLength = input
            .read_u32::<LittleEndian>()
            .map_err(|_| CCIDError::BadCommand)?;
        let bSlot = input.read_u8().map_err(|_| CCIDError::BadCommand)?;
        let bSeq = input.read_u8().map_err(|_| CCIDError::BadCommand)?;
        Ok(Self {
            bMessageType,
            dwLength,
            bSlot,
            bSeq,
        })
    }
}

impl Encode for CommonMessageHeader {
    type Error = ();
    fn encode<T: byteorder::WriteBytesExt>(&self, out: &mut T) -> Result<(), Self::Error> {
        out.write_u8(self.bMessageType)
            .expect("CommonMessageHeader: Failed to write message type");
        out.write_u32::<LittleEndian>(self.dwLength)
            .expect("CommonMessageHeader: Failed to write message length");
        out.write_u8(self.bSlot)
            .expect("CommonMessageHeader: Failed to write slot id");
        out.write_u8(self.bSeq)
            .expect("CommonMessageHeader: Failed to write sequence number");
        Ok(())
    }
}

#[derive(Debug, Copy, Clone)]
pub enum ICCVoltage {
    AUTO,
    V_5_0, // 5V
    V_3_0, // 3V
    V_1_8, // 1.8V
}

pub trait Decode {
    type Error;
    fn decode<T: byteorder::ReadBytesExt>(input: &mut T) -> Result<Self, Self::Error>
    where
        Self: Sized;
}

impl From<ICCVoltage> for u8 {
    fn from(value: ICCVoltage) -> Self {
        match value {
            ICCVoltage::AUTO => 0x00,
            ICCVoltage::V_5_0 => 0x01,
            ICCVoltage::V_3_0 => 0x02,
            ICCVoltage::V_1_8 => 0x03,
        }
    }
}

impl TryFrom<u8> for ICCVoltage {
    type Error = SlotErrorRegister;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x00 => Ok(ICCVoltage::AUTO),
            0x01 => Ok(ICCVoltage::V_5_0),
            0x02 => Ok(ICCVoltage::V_3_0),
            0x03 => Ok(ICCVoltage::V_1_8),
            // PC_to_RDR_IccPowerOn
            _ => Err(SlotErrorRegister::InvalidParameter(0x07)),
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum SlotErrorRegister {
    CommandAbort,
    ICCMute,
    TransferParityError,
    TransferOverrun,
    HardwareError,
    BadATRTS,
    BadATRTCK,
    UnsupportedICCProtocol,
    UnsupportedICCClass,
    ProcedureByteConflict,
    DeactivatedProtocol,
    BusyWithAutoSequence,
    PINTimeout,
    PINCancelled,
    CommandSlotBusy,
    UnsupportedCommand,
    UserDefined(u8),
    RFU(u8),
    InvalidParameter(u8),
}

impl From<SlotErrorRegister> for u8 {
    fn from(value: SlotErrorRegister) -> Self {
        match value {
            SlotErrorRegister::CommandAbort => ccid_const::CMD_ABORTED,
            SlotErrorRegister::ICCMute => ccid_const::ICC_MUTE,
            SlotErrorRegister::TransferParityError => ccid_const::XFR_PARITY_ERROR,
            SlotErrorRegister::TransferOverrun => ccid_const::XFR_OVERRUN,
            SlotErrorRegister::HardwareError => ccid_const::HW_ERROR,
            SlotErrorRegister::BadATRTS => ccid_const::BAD_ATR_TS,
            SlotErrorRegister::BadATRTCK => ccid_const::BAD_ATR_TCK,
            SlotErrorRegister::UnsupportedICCProtocol => ccid_const::ICC_PROTOCOL_NOT_SUPPORTED,
            SlotErrorRegister::UnsupportedICCClass => ccid_const::ICC_CLASS_NOT_SUPPORTED,
            SlotErrorRegister::ProcedureByteConflict => ccid_const::PROCEDURE_BYTE_CONFLICT,
            SlotErrorRegister::DeactivatedProtocol => ccid_const::DEACTIVATED_PROTOCOL,
            SlotErrorRegister::BusyWithAutoSequence => ccid_const::BUSY_WITH_AUTO_SEQUENCE,
            SlotErrorRegister::PINTimeout => ccid_const::PIN_TIMEOUT,
            SlotErrorRegister::PINCancelled => ccid_const::PIN_CANCELLED,
            SlotErrorRegister::CommandSlotBusy => ccid_const::CMD_SLOT_BUSY,
            SlotErrorRegister::UnsupportedCommand => ccid_const::CMD_NOT_SUPPORTED,
            SlotErrorRegister::UserDefined(x) => x,
            SlotErrorRegister::RFU(x) => x,
            SlotErrorRegister::InvalidParameter(x) => x,
        }
    }
}

impl From<u8> for SlotErrorRegister {
    fn from(value: u8) -> Self {
        #[allow(unreachable_patterns)]
        match value {
            ccid_const::CMD_ABORTED => SlotErrorRegister::CommandAbort,
            ccid_const::ICC_MUTE => SlotErrorRegister::ICCMute,
            ccid_const::XFR_PARITY_ERROR => SlotErrorRegister::TransferParityError,
            ccid_const::XFR_OVERRUN => SlotErrorRegister::TransferOverrun,
            ccid_const::HW_ERROR => SlotErrorRegister::HardwareError,
            ccid_const::BAD_ATR_TS => SlotErrorRegister::BadATRTS,
            ccid_const::BAD_ATR_TCK => SlotErrorRegister::BadATRTCK,
            ccid_const::ICC_PROTOCOL_NOT_SUPPORTED => SlotErrorRegister::UnsupportedICCProtocol,
            ccid_const::ICC_CLASS_NOT_SUPPORTED => SlotErrorRegister::UnsupportedICCClass,
            ccid_const::PROCEDURE_BYTE_CONFLICT => SlotErrorRegister::ProcedureByteConflict,
            ccid_const::DEACTIVATED_PROTOCOL => SlotErrorRegister::DeactivatedProtocol,
            ccid_const::BUSY_WITH_AUTO_SEQUENCE => SlotErrorRegister::BusyWithAutoSequence,
            ccid_const::PIN_TIMEOUT => SlotErrorRegister::PINTimeout,
            ccid_const::PIN_CANCELLED => SlotErrorRegister::PINCancelled,
            ccid_const::CMD_SLOT_BUSY => SlotErrorRegister::CommandSlotBusy,
            ccid_const::CMD_NOT_SUPPORTED => SlotErrorRegister::UnsupportedCommand,
            0x81..=0xC0 => SlotErrorRegister::UserDefined(value),
            0x01..=0x7F => SlotErrorRegister::InvalidParameter(value),
            0xF9 | 0xFA | 0xF1 | 0xE1..=0xEE | 0x80 | 0xC1..=0xDF => SlotErrorRegister::RFU(value),
            _ => unreachable!(),
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum ICCStatus {
    Active,
    Inactive,
    Absent,
    // RFU
}

impl ICCStatus {
    const fn to_u8(self) -> u8 {
        match self {
            ICCStatus::Active => 0x00,
            ICCStatus::Inactive => 0x01,
            ICCStatus::Absent => 0x02,
        }
    }
}

impl From<ICCStatus> for u8 {
    fn from(value: ICCStatus) -> Self {
        value.to_u8()
    }
}

impl TryFrom<u8> for ICCStatus {
    type Error = ();
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x00 => Ok(ICCStatus::Active),
            0x01 => Ok(ICCStatus::Inactive),
            0x02 => Ok(ICCStatus::Absent),
            _ => panic!("invalid ICC status value {}", value),
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum CommandStatus {
    Success,
    Failure,
    TimeExtensionRequested,
    // RFU
}

impl CommandStatus {
    const fn to_u8(self) -> u8 {
        match self {
            CommandStatus::Success => 0x00,
            CommandStatus::Failure => 0x01,
            CommandStatus::TimeExtensionRequested => 0x02,
        }
    }
}

impl From<CommandStatus> for u8 {
    fn from(value: CommandStatus) -> Self {
        value.to_u8()
    }
}

impl TryFrom<u8> for CommandStatus {
    type Error = ();
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x00 => Ok(CommandStatus::Success),
            0x01 => Ok(CommandStatus::Failure),
            0x02 => Ok(CommandStatus::TimeExtensionRequested),
            _ => panic!("invalid command status value {}", value),
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub enum SlotStatusRegister {
    ICCActiveSuccess,
    ICCActiveFailure,
    ICCActiveTimeExtensionRequested,
    // ICCPresentActive | RFU
    ICCInactiveSuccess,
    ICCInactiveFailure,
    ICCInactiveTimeExtensionRequested,
    // ICCPresentInactive | RFU
    ICCAbsentSuccess,
    ICCAbsentFailure,
    ICCAbsentTimeExtensionRequested,
    // RFU | RFU
}

#[allow(dead_code)]
impl SlotStatusRegister {
    pub fn ICCStatus(self) -> ICCStatus {
        ICCStatus::try_from(Into::<u8>::into(self) & 0x03).unwrap()
    }

    pub fn CommandStatus(self) -> CommandStatus {
        CommandStatus::try_from((Into::<u8>::into(self) & 0xC0) >> 6).unwrap()
    }
}

const fn combine_slot_status(icc: ICCStatus, command: CommandStatus) -> u8 {
    icc.to_u8() | (command.to_u8() << 6)
}

impl From<SlotStatusRegister> for u8 {
    fn from(value: SlotStatusRegister) -> Self {
        match value {
            SlotStatusRegister::ICCActiveSuccess => {
                combine_slot_status(ICCStatus::Active, CommandStatus::Success)
            }
            SlotStatusRegister::ICCActiveFailure => {
                combine_slot_status(ICCStatus::Active, CommandStatus::Failure)
            }
            SlotStatusRegister::ICCActiveTimeExtensionRequested => {
                combine_slot_status(ICCStatus::Active, CommandStatus::TimeExtensionRequested)
            }
            SlotStatusRegister::ICCInactiveSuccess => {
                combine_slot_status(ICCStatus::Inactive, CommandStatus::Success)
            }
            SlotStatusRegister::ICCInactiveFailure => {
                combine_slot_status(ICCStatus::Inactive, CommandStatus::Failure)
            }
            SlotStatusRegister::ICCInactiveTimeExtensionRequested => {
                combine_slot_status(ICCStatus::Inactive, CommandStatus::TimeExtensionRequested)
            }
            SlotStatusRegister::ICCAbsentSuccess => {
                combine_slot_status(ICCStatus::Absent, CommandStatus::Success)
            }
            SlotStatusRegister::ICCAbsentFailure => {
                combine_slot_status(ICCStatus::Absent, CommandStatus::Failure)
            }
            SlotStatusRegister::ICCAbsentTimeExtensionRequested => {
                combine_slot_status(ICCStatus::Absent, CommandStatus::TimeExtensionRequested)
            }
        }
    }
}

impl TryFrom<u8> for SlotStatusRegister {
    type Error = ();
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            _ if value == combine_slot_status(ICCStatus::Active, CommandStatus::Success) => {
                Ok(SlotStatusRegister::ICCActiveSuccess)
            }
            _ if value == combine_slot_status(ICCStatus::Active, CommandStatus::Failure) => {
                Ok(SlotStatusRegister::ICCActiveFailure)
            }
            _ if value
                == combine_slot_status(
                    ICCStatus::Active,
                    CommandStatus::TimeExtensionRequested,
                ) =>
            {
                Ok(SlotStatusRegister::ICCActiveTimeExtensionRequested)
            }
            _ if value == combine_slot_status(ICCStatus::Inactive, CommandStatus::Success) => {
                Ok(SlotStatusRegister::ICCInactiveSuccess)
            }
            _ if value == combine_slot_status(ICCStatus::Inactive, CommandStatus::Failure) => {
                Ok(SlotStatusRegister::ICCInactiveFailure)
            }
            _ if value
                == combine_slot_status(
                    ICCStatus::Inactive,
                    CommandStatus::TimeExtensionRequested,
                ) =>
            {
                Ok(SlotStatusRegister::ICCInactiveTimeExtensionRequested)
            }
            _ if value == combine_slot_status(ICCStatus::Absent, CommandStatus::Success) => {
                Ok(SlotStatusRegister::ICCAbsentSuccess)
            }
            _ if value == combine_slot_status(ICCStatus::Absent, CommandStatus::Failure) => {
                Ok(SlotStatusRegister::ICCAbsentFailure)
            }
            _ if value
                == combine_slot_status(
                    ICCStatus::Absent,
                    CommandStatus::TimeExtensionRequested,
                ) =>
            {
                Ok(SlotStatusRegister::ICCAbsentTimeExtensionRequested)
            }
            _ => panic!("invalid slot status value {}", value),
        }
    }
}

// #[derive(Debug, Clone, Copy)]
// enum DataBlockLevelParameter {
//     APDUNoChaining,
//     APDUChainedWithTransferCommand,
//     LastChainedAPDUWithData,
//     ChainedAPDUWithData,
//     ChainedWithNextDataBlockResponse,
// }
//
// impl Into<u16> for DataBlockLevelParameter {
//     fn into(self) -> u16 {
//         match self {
//             // same as RFU ( 00h ) if T=0 or TPDU or short APDU
//             DataBlockLevelParameter::APDUNoChaining => 0x00,
//             DataBlockLevelParameter::APDUChainedWithTransferCommand => 0x01,
//             DataBlockLevelParameter::LastChainedAPDUWithData => 0x02,
//             DataBlockLevelParameter::ChainedAPDUWithData => 0x03,
//             DataBlockLevelParameter::ChainedWithNextDataBlockResponse => 0x10,
//         }
//     }
// }

#[derive(Debug, Copy, Clone)]
pub enum ICCProtocol {
    T0,
    T1,
}

impl From<ICCProtocol> for u8 {
    fn from(value: ICCProtocol) -> Self {
        match value {
            ICCProtocol::T0 => 0x00,
            ICCProtocol::T1 => 0x01,
        }
    }
}

impl TryFrom<u8> for ICCProtocol {
    type Error = SlotErrorRegister;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x00 => Ok(ICCProtocol::T0),
            0x01 => Ok(ICCProtocol::T1),
            // RDR_to_PC_Parameters
            _ => Err(SlotErrorRegister::InvalidParameter(0x9)),
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub enum ICCClockCommand {
    Restart,
    Stop,
}

impl From<ICCClockCommand> for u8 {
    fn from(value: ICCClockCommand) -> Self {
        match value {
            ICCClockCommand::Restart => 0x00,
            ICCClockCommand::Stop => 0x01,
        }
    }
}

impl TryFrom<u8> for ICCClockCommand {
    type Error = SlotErrorRegister;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x00 => Ok(ICCClockCommand::Restart),
            0x01 => Ok(ICCClockCommand::Stop),
            // PC_to_RDR_IccClock
            _ => Err(SlotErrorRegister::InvalidParameter(0x07)),
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub enum T0APDUClassChange {
    None,
    GetResponse,
    Envelope,
    Both,
}

impl From<T0APDUClassChange> for u8 {
    fn from(value: T0APDUClassChange) -> Self {
        match value {
            T0APDUClassChange::None => 0x00,
            T0APDUClassChange::GetResponse => 0x01,
            T0APDUClassChange::Envelope => 0x02,
            T0APDUClassChange::Both => 0x03,
        }
    }
}

impl TryFrom<u8> for T0APDUClassChange {
    type Error = SlotErrorRegister;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x00 => Ok(T0APDUClassChange::None),
            0x01 => Ok(T0APDUClassChange::GetResponse),
            0x02 => Ok(T0APDUClassChange::Envelope),
            0x03 => Ok(T0APDUClassChange::Both),
            // PC_to_RDR_T0APDU
            _ => Err(SlotErrorRegister::InvalidParameter(0x07)),
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub enum ICCMechanicalFunction {
    AcceptCard,
    EjectCard,
    CaptureCard,
    LockCard,
    UnlockCard,
}

impl From<ICCMechanicalFunction> for u8 {
    fn from(value: ICCMechanicalFunction) -> Self {
        match value {
            ICCMechanicalFunction::AcceptCard => 0x01,
            ICCMechanicalFunction::EjectCard => 0x02,
            ICCMechanicalFunction::CaptureCard => 0x03,
            ICCMechanicalFunction::LockCard => 0x04,
            ICCMechanicalFunction::UnlockCard => 0x05,
        }
    }
}

impl TryFrom<u8> for ICCMechanicalFunction {
    type Error = SlotErrorRegister;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x01 => Ok(ICCMechanicalFunction::AcceptCard),
            0x02 => Ok(ICCMechanicalFunction::EjectCard),
            0x03 => Ok(ICCMechanicalFunction::CaptureCard),
            0x04 => Ok(ICCMechanicalFunction::LockCard),
            0x05 => Ok(ICCMechanicalFunction::UnlockCard),
            // PC_to_RDR_Mechanical
            _ => Err(SlotErrorRegister::InvalidParameter(0x07)),
        }
    }
}

#[allow(dead_code)]
#[derive(Debug)]
pub enum Command {
    PC_to_RDR_IccPowerOn {
        header: CommonMessageHeader, //<{ ccid_const::PC_to_RDR_IccPowerOn }>,
        bPowerSelect: ICCVoltage,
        abRFU: [u8; 2],
    },
    PC_to_RDR_IccPowerOff {
        header: CommonMessageHeader, // <{ ccid_const::PC_to_RDR_IccPowerOff }>,
        abRFU: [u8; 3],
    },
    PC_to_RDR_GetSlotStatus {
        header: CommonMessageHeader, //<{ ccid_const::PC_to_RDR_GetSlotStatus }>,
        abRFU: [u8; 3],
    },
    PC_to_RDR_XfrBlock {
        header: CommonMessageHeader, //<{ ccid_const::PC_to_RDR_XfrBlock }>,
        bBWI: u8,                    // unit per Block waiting time
        wLevelParameter: u16,
        abData: Vec<u8>,
    },
    PC_to_RDR_GetParameters {
        header: CommonMessageHeader, // <{ ccid_const::PC_to_RDR_GetParameters }>,
        abRFU: [u8; 3],
    },
    PC_to_RDR_ResetParameters {
        header: CommonMessageHeader, //<{ ccid_const::PC_to_RDR_ResetParameters }>,
        abRFU: [u8; 3],
    },
    PC_to_RDR_SetParameters {
        header: CommonMessageHeader, //<{ ccid_const::PC_to_RDR_SetParameters }>,
        bProtocolNum: ICCProtocol,
        abRFU: [u8; 2],
        abData: Vec<u8>,
        //abProtocolDataStructure : Option<i32>
    },
    PC_to_RDR_Escape {
        header: CommonMessageHeader, //<{ ccid_const::PC_to_RDR_Escape }>,
        abRFU: [u8; 3],
        abData: Vec<u8>,
    },
    PC_to_RDR_IccClock {
        header: CommonMessageHeader, // <{ ccid_const::PC_to_RDR_IccClock }>,
        bClockCommand: ICCClockCommand,
        abRFU: [u8; 2],
    },
    PC_to_RDR_T0APDU {
        header: CommonMessageHeader, //<{ ccid_const::PC_to_RDR_T0APDU }>,
        bmChanges: T0APDUClassChange,
        bClassGetResponse: u8,
        bClassEnvelope: u8,
    },
    PC_to_RDR_Secure {
        header: CommonMessageHeader, // <{ ccid_const::PC_to_RDR_Secure }>,
        bBWI: u8,                    // unit per Block waiting time
        wLevelParameter: u16,
        abData: Vec<u8>,
    },
    PC_to_RDR_Mechanical {
        header: CommonMessageHeader, // <{ ccid_const::PC_to_RDR_Mechanical }>,
        bFunction: ICCMechanicalFunction,
        abRFU: [u8; 2],
    },
    PC_to_RDR_Abort {
        header: CommonMessageHeader, // <{ ccid_const::PC_to_RDR_Abort }>,
        abRFU: [u8; 3],
    },
    PC_to_RDR_SetDataRateAndClockFrequency {
        header: CommonMessageHeader, // <{ ccid_const::PC_to_RDR_SetDataRateAndClockFrequency }>,
        abRFU: [u8; 3],
        dwClockFrequency: u32, // in KHz
        dwDataRate: u32,       // in bpd
    },
}

impl Decode for Command {
    type Error = CCIDError;
    fn decode<T: byteorder::ReadBytesExt>(input: &mut T) -> Result<Self, Self::Error> {
        let header = CommonMessageHeader::decode(input)?;
        let command = match header.bMessageType {
            ccid_const::PC_to_RDR_IccPowerOn => {
                if header.dwLength != 0 {
                    return Err(CCIDError::command_error(
                        header,
                        SlotStatusRegister::ICCInactiveFailure,
                        SlotErrorRegister::InvalidParameter(0x1),
                    ));
                }
                let bPowerSelect = ICCVoltage::try_from(input.read_u8().map_err(|_| {
                    CCIDError::command_error(
                        header,
                        SlotStatusRegister::ICCInactiveFailure,
                        SlotErrorRegister::InvalidParameter(0x7),
                    )
                })?)
                .map_err(|e| {
                    CCIDError::command_error(header, SlotStatusRegister::ICCInactiveFailure, e)
                })?;
                let mut abRFU = [0u8; 2];
                input.read_exact(&mut abRFU).map_err(|_| {
                    CCIDError::command_error(
                        header,
                        SlotStatusRegister::ICCInactiveFailure,
                        SlotErrorRegister::InvalidParameter(0x8),
                    )
                })?;
                Self::PC_to_RDR_IccPowerOn {
                    header,
                    bPowerSelect,
                    abRFU,
                }
            }
            ccid_const::PC_to_RDR_IccPowerOff => {
                if header.dwLength != 0 {
                    return Err(CCIDError::command_error(
                        header,
                        SlotStatusRegister::ICCInactiveFailure,
                        SlotErrorRegister::InvalidParameter(0x1),
                    ));
                }
                let mut abRFU = [0u8; 3];
                input.read_exact(&mut abRFU).map_err(|_| {
                    CCIDError::command_error(
                        header,
                        SlotStatusRegister::ICCInactiveFailure,
                        SlotErrorRegister::InvalidParameter(0x7),
                    )
                })?;
                Self::PC_to_RDR_IccPowerOff { header, abRFU }
            }
            ccid_const::PC_to_RDR_GetSlotStatus => {
                if header.dwLength != 0 {
                    return Err(CCIDError::command_error(
                        header,
                        SlotStatusRegister::ICCInactiveFailure,
                        SlotErrorRegister::InvalidParameter(0x1),
                    ));
                }
                let mut abRFU = [0u8; 3];
                input.read_exact(&mut abRFU).map_err(|_| {
                    CCIDError::command_error(
                        header,
                        SlotStatusRegister::ICCInactiveFailure,
                        SlotErrorRegister::InvalidParameter(0x7),
                    )
                })?;
                Self::PC_to_RDR_GetSlotStatus { header, abRFU }
            }
            ccid_const::PC_to_RDR_XfrBlock => {
                let bBWI = input.read_u8().map_err(|_| {
                    CCIDError::command_error(
                        header,
                        SlotStatusRegister::ICCInactiveFailure,
                        SlotErrorRegister::InvalidParameter(0x7),
                    )
                })?;
                let wLevelParameter = input.read_u16::<LittleEndian>().map_err(|_| {
                    CCIDError::command_error(
                        header,
                        SlotStatusRegister::ICCInactiveFailure,
                        SlotErrorRegister::InvalidParameter(0x8),
                    )
                })?;
                let mut abData = vec![0u8; header.dwLength as usize];
                input.read_exact(&mut abData).map_err(|_| {
                    CCIDError::command_error(
                        header,
                        SlotStatusRegister::ICCInactiveFailure,
                        SlotErrorRegister::InvalidParameter(0x1),
                    )
                })?;
                Self::PC_to_RDR_XfrBlock {
                    header,
                    bBWI,
                    wLevelParameter,
                    abData,
                }
            }
            ccid_const::PC_to_RDR_GetParameters => {
                if header.dwLength != 0 {
                    return Err(CCIDError::command_error(
                        header,
                        SlotStatusRegister::ICCInactiveFailure,
                        SlotErrorRegister::InvalidParameter(0x1),
                    ));
                }
                let mut abRFU = [0u8; 3];
                input.read_exact(&mut abRFU).map_err(|_| {
                    CCIDError::command_error(
                        header,
                        SlotStatusRegister::ICCInactiveFailure,
                        SlotErrorRegister::InvalidParameter(0x7),
                    )
                })?;
                Self::PC_to_RDR_GetParameters { header, abRFU }
            }
            ccid_const::PC_to_RDR_ResetParameters => {
                if header.dwLength != 0 {
                    return Err(CCIDError::command_error(
                        header,
                        SlotStatusRegister::ICCInactiveFailure,
                        SlotErrorRegister::InvalidParameter(0x1),
                    ));
                }
                let mut abRFU = [0u8; 3];
                input.read_exact(&mut abRFU).map_err(|_| {
                    CCIDError::command_error(
                        header,
                        SlotStatusRegister::ICCInactiveFailure,
                        SlotErrorRegister::InvalidParameter(0x7),
                    )
                })?;
                Self::PC_to_RDR_ResetParameters { header, abRFU }
            }
            ccid_const::PC_to_RDR_SetParameters => {
                let bProtocolNum = ICCProtocol::try_from(input.read_u8().map_err(|_| {
                    CCIDError::command_error(
                        header,
                        SlotStatusRegister::ICCInactiveFailure,
                        SlotErrorRegister::InvalidParameter(0x7),
                    )
                })?)
                .map_err(|e| {
                    CCIDError::command_error(header, SlotStatusRegister::ICCInactiveFailure, e)
                })?;
                let mut abRFU = [0u8; 2];
                input.read_exact(&mut abRFU).map_err(|_| {
                    CCIDError::command_error(
                        header,
                        SlotStatusRegister::ICCInactiveFailure,
                        SlotErrorRegister::InvalidParameter(0x8),
                    )
                })?;
                let mut abData = vec![0u8; header.dwLength as usize];
                input.read_exact(&mut abData).map_err(|_| {
                    CCIDError::command_error(
                        header,
                        SlotStatusRegister::ICCInactiveFailure,
                        SlotErrorRegister::InvalidParameter(0x1),
                    )
                })?;
                Self::PC_to_RDR_SetParameters {
                    header,
                    bProtocolNum,
                    abRFU,
                    abData,
                }
            }
            ccid_const::PC_to_RDR_Escape => {
                let mut abRFU = [0u8; 3];
                input.read_exact(&mut abRFU).map_err(|_| {
                    CCIDError::command_error(
                        header,
                        SlotStatusRegister::ICCInactiveFailure,
                        SlotErrorRegister::InvalidParameter(0x7),
                    )
                })?;
                let mut abData = vec![0u8; header.dwLength as usize];
                input.read_exact(&mut abData).map_err(|_| {
                    CCIDError::command_error(
                        header,
                        SlotStatusRegister::ICCInactiveFailure,
                        SlotErrorRegister::InvalidParameter(0x1),
                    )
                })?;
                Self::PC_to_RDR_Escape {
                    header,
                    abRFU,
                    abData,
                }
            }
            ccid_const::PC_to_RDR_IccClock => {
                if header.dwLength != 0 {
                    return Err(CCIDError::command_error(
                        header,
                        SlotStatusRegister::ICCInactiveFailure,
                        SlotErrorRegister::InvalidParameter(0x1),
                    ));
                }
                let bClockCommand = ICCClockCommand::try_from(input.read_u8().map_err(|_| {
                    CCIDError::command_error(
                        header,
                        SlotStatusRegister::ICCInactiveFailure,
                        SlotErrorRegister::InvalidParameter(0x7),
                    )
                })?)
                .map_err(|e| {
                    CCIDError::command_error(header, SlotStatusRegister::ICCInactiveFailure, e)
                })?;
                let mut abRFU = [0u8; 2];
                input.read_exact(&mut abRFU).map_err(|_| {
                    CCIDError::command_error(
                        header,
                        SlotStatusRegister::ICCInactiveFailure,
                        SlotErrorRegister::InvalidParameter(0x8),
                    )
                })?;
                Self::PC_to_RDR_IccClock {
                    header,
                    bClockCommand,
                    abRFU,
                }
            }
            ccid_const::PC_to_RDR_T0APDU => {
                if header.dwLength != 0 {
                    return Err(CCIDError::command_error(
                        header,
                        SlotStatusRegister::ICCInactiveFailure,
                        SlotErrorRegister::InvalidParameter(0x1),
                    ));
                }
                let bmChanges = T0APDUClassChange::try_from(input.read_u8().map_err(|_| {
                    CCIDError::command_error(
                        header,
                        SlotStatusRegister::ICCInactiveFailure,
                        SlotErrorRegister::InvalidParameter(0x7),
                    )
                })?)
                .map_err(|e| {
                    CCIDError::command_error(header, SlotStatusRegister::ICCInactiveFailure, e)
                })?;
                let bClassGetResponse = input.read_u8().map_err(|_| {
                    CCIDError::command_error(
                        header,
                        SlotStatusRegister::ICCInactiveFailure,
                        SlotErrorRegister::InvalidParameter(0x8),
                    )
                })?;
                let bClassEnvelope = input.read_u8().map_err(|_| {
                    CCIDError::command_error(
                        header,
                        SlotStatusRegister::ICCInactiveFailure,
                        SlotErrorRegister::InvalidParameter(0x9),
                    )
                })?;
                Self::PC_to_RDR_T0APDU {
                    header,
                    bmChanges,
                    bClassGetResponse,
                    bClassEnvelope,
                }
            }
            ccid_const::PC_to_RDR_Secure => {
                let bBWI = input.read_u8().map_err(|_| {
                    CCIDError::command_error(
                        header,
                        SlotStatusRegister::ICCInactiveFailure,
                        SlotErrorRegister::InvalidParameter(0x7),
                    )
                })?;
                let wLevelParameter = input.read_u16::<LittleEndian>().map_err(|_| {
                    CCIDError::command_error(
                        header,
                        SlotStatusRegister::ICCInactiveFailure,
                        SlotErrorRegister::InvalidParameter(0x8),
                    )
                })?;
                let mut abData = vec![0u8; header.dwLength as usize];
                input.read_exact(&mut abData).map_err(|_| {
                    CCIDError::command_error(
                        header,
                        SlotStatusRegister::ICCInactiveFailure,
                        SlotErrorRegister::InvalidParameter(0x1),
                    )
                })?;
                Self::PC_to_RDR_Secure {
                    header,
                    bBWI,
                    wLevelParameter,
                    abData,
                }
            }
            ccid_const::PC_to_RDR_Mechanical => {
                if header.dwLength != 0 {
                    return Err(CCIDError::command_error(
                        header,
                        SlotStatusRegister::ICCInactiveFailure,
                        SlotErrorRegister::InvalidParameter(0x1),
                    ));
                }
                let bFunction = ICCMechanicalFunction::try_from(input.read_u8().map_err(|_| {
                    CCIDError::command_error(
                        header,
                        SlotStatusRegister::ICCInactiveFailure,
                        SlotErrorRegister::InvalidParameter(0x7),
                    )
                })?)
                .map_err(|e| {
                    CCIDError::command_error(header, SlotStatusRegister::ICCInactiveFailure, e)
                })?;
                let mut abRFU = [0u8; 2];
                input.read_exact(&mut abRFU).map_err(|_| {
                    CCIDError::command_error(
                        header,
                        SlotStatusRegister::ICCInactiveFailure,
                        SlotErrorRegister::InvalidParameter(0x8),
                    )
                })?;
                Self::PC_to_RDR_Mechanical {
                    header,
                    bFunction,
                    abRFU,
                }
            }
            ccid_const::PC_to_RDR_Abort => {
                if header.dwLength != 0 {
                    return Err(CCIDError::command_error(
                        header,
                        SlotStatusRegister::ICCInactiveFailure,
                        SlotErrorRegister::InvalidParameter(0x1),
                    ));
                }
                let mut abRFU = [0u8; 3];
                input.read_exact(&mut abRFU).map_err(|_| {
                    CCIDError::command_error(
                        header,
                        SlotStatusRegister::ICCInactiveFailure,
                        SlotErrorRegister::InvalidParameter(0x7),
                    )
                })?;
                Self::PC_to_RDR_Abort { header, abRFU }
            }
            ccid_const::PC_to_RDR_SetDataRateAndClockFrequency => {
                if header.dwLength != 8 {
                    return Err(CCIDError::command_error(
                        header,
                        SlotStatusRegister::ICCInactiveFailure,
                        SlotErrorRegister::InvalidParameter(0x1),
                    ));
                }
                let mut abRFU = [0u8; 3];
                input.read_exact(&mut abRFU).map_err(|_| {
                    CCIDError::command_error(
                        header,
                        SlotStatusRegister::ICCInactiveFailure,
                        SlotErrorRegister::InvalidParameter(0x7),
                    )
                })?;
                let dwClockFrequency = input.read_u32::<LittleEndian>().map_err(|_| {
                    CCIDError::command_error(
                        header,
                        SlotStatusRegister::ICCInactiveFailure,
                        SlotErrorRegister::InvalidParameter(0xa),
                    )
                })?;
                let dwDataRate = input.read_u32::<LittleEndian>().map_err(|_| {
                    CCIDError::command_error(
                        header,
                        SlotStatusRegister::ICCInactiveFailure,
                        SlotErrorRegister::InvalidParameter(0xe),
                    )
                })?;
                Self::PC_to_RDR_SetDataRateAndClockFrequency {
                    header,
                    abRFU,
                    dwClockFrequency,
                    dwDataRate,
                }
            }
            _ => {
                return Err(CCIDError::CommandError(ResponseMessageHeader::new(
                    header,
                    SlotStatusRegister::ICCActiveFailure,
                    SlotErrorRegister::UnsupportedCommand,
                )));
            }
        };
        if input.read_u8().is_ok() {
            return Err(CCIDError::command_error(
                header,
                SlotStatusRegister::ICCInactiveFailure,
                SlotErrorRegister::InvalidParameter(0x1),
            ));
        }
        Ok(command)
    }
}

#[derive(Debug, Copy, Clone)]
pub struct ResponseMessageHeader {
    inner: CommonMessageHeader,
    pub bStatus: SlotStatusRegister,
    pub bError: SlotErrorRegister,
}

impl Deref for ResponseMessageHeader {
    type Target = CommonMessageHeader;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for ResponseMessageHeader {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl ResponseMessageHeader {
    pub fn new(
        command: CommonMessageHeader,
        status: SlotStatusRegister,
        error: SlotErrorRegister,
    ) -> Self {
        Self {
            inner: command,
            bStatus: status,
            bError: error,
        }
    }
}

impl Encode for ResponseMessageHeader {
    type Error = ();
    fn encode<T: WriteBytesExt>(&self, out: &mut T) -> Result<(), Self::Error> {
        self.inner.encode(out)?;
        out.write_u8(self.bStatus.into())
            .expect("ResponseMessageHeader: Failed to write slot status");
        out.write_u8(self.bError.into())
            .expect("ResponseMessageHeader: Failed to write slot error");
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ICCClockStatus {
    Running,
    StoppedInL,
    StoppedInH,
    StoppedUnknown,
}

impl From<ICCClockStatus> for u8 {
    fn from(value: ICCClockStatus) -> Self {
        match value {
            ICCClockStatus::Running => 0x00,
            ICCClockStatus::StoppedInL => 0x01,
            ICCClockStatus::StoppedInH => 0x02,
            ICCClockStatus::StoppedUnknown => 0x03,
        }
    }
}

impl TryFrom<u8> for ICCClockStatus {
    type Error = ();
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x00 => Ok(ICCClockStatus::Running),
            0x01 => Ok(ICCClockStatus::StoppedInL),
            0x02 => Ok(ICCClockStatus::StoppedInH),
            0x03 => Ok(ICCClockStatus::StoppedUnknown),
            _ => panic!("invalid ICCClockStatus value {}", value),
        }
    }
}

// #[derive(Debug, Clone, Copy)]
// pub enum Fi {
//     F0
// }
//
// #[derive(Debug, Clone, Copy)]
// pub enum FIndex {
//     F0,
//     F1,
//     F2,
//     F3,
//     F4,
//     F5,
//     F6,
//     // F7, // RFU
//     // F8, // RFU
//     F9,
//     F10,
//     F11,
//     F12,
//     F13,
//     // F14, // RFU
//     // F15, // RFU
// }
//
// impl Into<u8> for FIndex {
//     fn into(self) -> u8 {
//         match self {
//             Self::F0 => 0x00,
//             Self::F1 => 0x01,
//             Self::F2 => 0x02,
//             Self::F3 => 0x03,
//             Self::F4 => 0x04,
//             Self::F5 => 0x05,
//             Self::F6 => 0x06,
//             Self::F9 => 0x09,
//             Self::F10 => 0x0A,
//             Self::F11 => 0x0B,
//             Self::F12 => 0x0C,
//             Self::F13 => 0x0D,
//         }
//     }
// }
//
// impl TryFrom<u8> for FIndex {
//     type Error = SlotErrorRegister;
//     fn try_from(value: u8) -> Result<Self, Self::Error> {
//         match value {
//             0x00 => Ok(FIndex::F0),
//             0x01 => Ok(FIndex::F1),
//             0x02 => Ok(FIndex::F2),
//             0x03 => Ok(FIndex::F3),
//             0x04 => Ok(FIndex::F4),
//             0x05 => Ok(FIndex::F5),
//             0x06 => Ok(FIndex::F6),
//             0x09 => Ok(FIndex::F9),
//             0x0A => Ok(FIndex::F10),
//             0x0B => Ok(FIndex::F11),
//             0x0C => Ok(FIndex::F12),
//             0x0D => Ok(FIndex::F13),
//             _ => Err(SlotErrorRegister::InvalidParameter(10))
//         }
//     }
// }
//
// impl FIndex {
//     pub fn from_fi_f(fi : u16, f : u8) ->
//     pub fn Fi() -> u16 {
//         match
//     }
// }
//
// #[derive(Debug, Clone, Copy)]
// pub enum DIndex {
//     //D0, // RFU
//     D1,
//     D2,
//     D3,
//     D4,
//     D5,
//     D6,
//     D7,
//     D8,
//     D9,
//     // D10, // RFU
//     // D11, // RFU
//     // D12, // RFU
//     // D13, // RFU
//     // D14, // RFU
//     // D15, // RFU
// }
//
// pub enum ProtocolData {
//     T0 {
//
//     },
//     T1 {
//     }
// }

#[derive(Debug)]
pub enum Response {
    RDR_to_PC_DataBlock {
        header: ResponseMessageHeader, //<{ ccid_const::RDR_to_PC_DataBlock }>,
        bChainParameter: u8,
        abData: Vec<u8>,
    },
    RDR_to_PC_SlotStatus {
        header: ResponseMessageHeader, //<{ ccid_const::RDR_to_PC_SlotStatus }>,
        bClockStatus: ICCClockStatus,
    },
    RDR_to_PC_Parameters {
        header: ResponseMessageHeader, //<{ ccid_const::RDR_to_PC_Parameters }>,
        bProtocolNum: ICCProtocol,
        abData: Vec<u8>,
    },
    RDR_to_PC_Escape {
        header: ResponseMessageHeader, //<{ ccid_const::RDR_to_PC_Escape }>,
        abData: Vec<u8>,
    },
    RDR_to_PC_DataRateAndClockFrequency {
        header: ResponseMessageHeader, //<{ ccid_const::RDR_to_PC_DataRateAndClockFrequency }>,
        dwClockFrequency: u32,
        dwDataRate: u32,
    },
    RDR_to_PC_UnsupportedCommand {
        header: ResponseMessageHeader,
    },
}

impl Command {
    pub fn get_header(&self) -> &CommonMessageHeader {
        match self {
            Self::PC_to_RDR_IccPowerOn {
                header, //<{ ccid_const::PC_to_RDR_IccPowerOn }>,
                bPowerSelect: _,
                abRFU: _,
            }
            | Self::PC_to_RDR_IccPowerOff {
                header, // <{ ccid_const::PC_to_RDR_IccPowerOff }>,
                abRFU: _,
            }
            | Self::PC_to_RDR_GetSlotStatus {
                header, //<{ ccid_const::PC_to_RDR_GetSlotStatus }>,
                abRFU: _,
            }
            | Self::PC_to_RDR_XfrBlock {
                header,  //<{ ccid_const::PC_to_RDR_XfrBlock }>,
                bBWI: _, // unit per Block waiting time
                wLevelParameter: _,
                abData: _,
            }
            | Self::PC_to_RDR_GetParameters {
                header, // <{ ccid_const::PC_to_RDR_GetParameters }>,
                abRFU: _,
            }
            | Self::PC_to_RDR_ResetParameters {
                header, //<{ ccid_const::PC_to_RDR_ResetParameters }>,
                abRFU: _,
            }
            | Self::PC_to_RDR_SetParameters {
                header, //<{ ccid_const::PC_to_RDR_SetParameters }>,
                bProtocolNum: _,
                abRFU: _,
                abData: _,
                //abProtocolDataStructure : Option<i32>
            }
            | Self::PC_to_RDR_Escape {
                header, //<{ ccid_const::PC_to_RDR_Escape }>,
                abRFU: _,
                abData: _,
            }
            | Self::PC_to_RDR_IccClock {
                header, // <{ ccid_const::PC_to_RDR_IccClock }>,
                bClockCommand: _,
                abRFU: _,
            }
            | Self::PC_to_RDR_T0APDU {
                header, //<{ ccid_const::PC_to_RDR_T0APDU }>,
                bmChanges: _,
                bClassGetResponse: _,
                bClassEnvelope: _,
            }
            | Self::PC_to_RDR_Secure {
                header,  // <{ ccid_const::PC_to_RDR_Secure }>,
                bBWI: _, // unit per Block waiting time
                wLevelParameter: _,
                abData: _,
            }
            | Self::PC_to_RDR_Mechanical {
                header, // <{ ccid_const::PC_to_RDR_Mechanical }>,
                bFunction: _,
                abRFU: _,
            }
            | Self::PC_to_RDR_Abort {
                header, // <{ ccid_const::PC_to_RDR_Abort }>,
                abRFU: _,
            }
            | Self::PC_to_RDR_SetDataRateAndClockFrequency {
                header, // <{ ccid_const::PC_to_RDR_SetDataRateAndClockFrequency }>,
                abRFU: _,
                dwClockFrequency: _, // in KHz
                dwDataRate: _,       // in bpd
            } => header,
        }
    }
}

impl Response {
    fn new_with_status(
        command: CommonMessageHeader,
        status: SlotStatusRegister,
        error: SlotErrorRegister,
    ) -> Self {
        let mut header = ResponseMessageHeader::new(command, status, error);
        header.dwLength = 0x0;
        if status.CommandStatus() == CommandStatus::Failure
            && error == SlotErrorRegister::UnsupportedCommand
        {
            return Self::RDR_to_PC_UnsupportedCommand { header };
        }
        match command.bMessageType {
            ccid_const::PC_to_RDR_IccPowerOn
            | ccid_const::PC_to_RDR_XfrBlock
            | ccid_const::PC_to_RDR_Secure => {
                header.bMessageType = ccid_const::RDR_to_PC_DataBlock;
                Self::RDR_to_PC_DataBlock {
                    header,
                    bChainParameter: 0,
                    abData: Vec::new(),
                }
            }
            ccid_const::PC_to_RDR_IccPowerOff
            | ccid_const::PC_to_RDR_GetSlotStatus
            | ccid_const::PC_to_RDR_IccClock
            | ccid_const::PC_to_RDR_T0APDU
            | ccid_const::PC_to_RDR_Mechanical
            | ccid_const::PC_to_RDR_Abort => {
                header.bMessageType = ccid_const::RDR_to_PC_SlotStatus;
                Self::RDR_to_PC_SlotStatus {
                    header,
                    bClockStatus: ICCClockStatus::Running,
                }
            }
            ccid_const::PC_to_RDR_GetParameters
            | ccid_const::PC_to_RDR_ResetParameters
            | ccid_const::PC_to_RDR_SetParameters => {
                header.bMessageType = ccid_const::RDR_to_PC_Parameters;
                Self::RDR_to_PC_Parameters {
                    header,
                    bProtocolNum: ICCProtocol::T1,
                    abData: Vec::new(),
                }
            }
            ccid_const::RDR_to_PC_Escape => {
                header.bMessageType = ccid_const::RDR_to_PC_Escape;
                Self::RDR_to_PC_Escape {
                    header,
                    abData: Vec::new(),
                }
            }
            ccid_const::PC_to_RDR_SetDataRateAndClockFrequency => {
                header.bMessageType = ccid_const::RDR_to_PC_DataRateAndClockFrequency;
                Self::RDR_to_PC_DataRateAndClockFrequency {
                    header,
                    dwDataRate: 0x0,
                    dwClockFrequency: 0x0,
                }
            }
            _ => {
                header.bStatus = SlotStatusRegister::ICCActiveFailure;
                header.bError = SlotErrorRegister::UnsupportedCommand;
                Self::RDR_to_PC_UnsupportedCommand { header }
            }
        }
    }

    pub fn new(command: CommonMessageHeader) -> Self {
        //let mut header = ResponseMessageHeader::new(command, SlotStatusRegister::ICCActiveSuccess, 0u8.into());
        Self::new_with_status(
            command,
            SlotStatusRegister::ICCActiveSuccess,
            SlotErrorRegister::UnsupportedCommand,
        )
    }

    pub fn new_with_error(header: ResponseMessageHeader) -> Self {
        Self::new_with_status(header.inner, header.bStatus, header.bError)
    }

    pub fn set_status(&mut self, status: SlotStatusRegister, error: SlotErrorRegister) {
        match self {
            Self::RDR_to_PC_SlotStatus { header, .. }
            | Self::RDR_to_PC_Parameters { header, .. }
            | Self::RDR_to_PC_DataBlock { header, .. }
            | Self::RDR_to_PC_DataRateAndClockFrequency { header, .. }
            | Self::RDR_to_PC_Escape { header, .. } => {
                header.bStatus = status;
                header.bError = error;
            }
            Self::RDR_to_PC_UnsupportedCommand { .. } => {
                debug!("Unsupported command should not be able to set custom status");
            }
        }
    }

    pub fn append(&mut self, data: &[u8]) -> Result<(), ()> {
        match self {
            Self::RDR_to_PC_DataBlock {
                abData,
                header,
                bChainParameter: _,
            }
            | Self::RDR_to_PC_Parameters {
                header,
                bProtocolNum: _,
                abData,
            }
            | Self::RDR_to_PC_Escape { header, abData } => {
                abData.extend_from_slice(data);
                header.dwLength += data.len() as u32;
                Ok(())
            }
            _ => Err(()),
        }
    }
}

pub trait Encode {
    type Error;
    fn encode<T: byteorder::WriteBytesExt>(&self, out: &mut T) -> Result<(), Self::Error>;
}

impl Encode for Response {
    type Error = ();
    fn encode<T: byteorder::WriteBytesExt>(&self, out: &mut T) -> Result<(), Self::Error> {
        match self {
            Self::RDR_to_PC_DataBlock {
                header,
                bChainParameter,
                abData,
            } => {
                header.encode(out)?;
                out.write_u8(*bChainParameter)
                    .expect("RDR_to_PC_DataBlock: Failed to write bChainParameter");
                out.write_all(abData)
                    .expect("RDR_to_PC_DataBlock: Failed to write abData");
            }
            Self::RDR_to_PC_SlotStatus {
                header,
                bClockStatus,
            } => {
                header.encode(out)?;
                out.write_u8((*bClockStatus).into())
                    .expect("RDR_to_PC_SlotStatus: Failed to write bClockStatus");
            }
            Self::RDR_to_PC_Parameters {
                header,
                bProtocolNum,
                abData,
            } => {
                header.encode(out)?;
                out.write_u8((*bProtocolNum).into())
                    .expect("RDR_to_PC_Parameters: Failed to write bProtocolNum");
                out.write_all(abData)
                    .expect("RDR_to_PC_Parameters: Failed to write abData");
            }
            Self::RDR_to_PC_Escape { header, abData } => {
                header.encode(out)?;
                out.write_all(abData)
                    .expect("RDR_to_PC_Escape: Failed to write abData");
            }
            Self::RDR_to_PC_DataRateAndClockFrequency {
                header,
                dwDataRate,
                dwClockFrequency,
            } => {
                header.encode(out)?;
                out.write_u32::<LittleEndian>(*dwDataRate)
                    .expect("RDR_to_PC_DataRateAndClockFrequency: Failed to write dwDataRate");
                out.write_u32::<LittleEndian>(*dwClockFrequency).expect(
                    "RDR_to_PC_DataRateAndClockFrequency: Failed to write dwClockFrequency",
                );
            }
            Self::RDR_to_PC_UnsupportedCommand { header } => {
                header.encode(out)?;
                out.write_u8(0u8)
                    .expect("RDR_to_PC_UnsupportedCommand: Failed to write RFU");
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {}
