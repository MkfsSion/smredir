// Command messages

pub const PC_to_RDR_IccPowerOn: u8 = 0x62;
pub const PC_to_RDR_IccPowerOff: u8 = 0x63;
pub const PC_to_RDR_GetSlotStatus: u8 = 0x65;
pub const PC_to_RDR_XfrBlock: u8 = 0x6F;
pub const PC_to_RDR_GetParameters: u8 = 0x6C;
pub const PC_to_RDR_ResetParameters: u8 = 0x6D;
pub const PC_to_RDR_SetParameters: u8 = 0x61;
pub const PC_to_RDR_Escape: u8 = 0x6B;
pub const PC_to_RDR_IccClock: u8 = 0x6E;
pub const PC_to_RDR_T0APDU: u8 = 0x6A;
pub const PC_to_RDR_Secure: u8 = 0x69;
pub const PC_to_RDR_Mechanical: u8 = 0x71;
pub const PC_to_RDR_Abort: u8 = 0x72;
pub const PC_to_RDR_SetDataRateAndClockFrequency: u8 = 0x73;

// Response messages
pub const RDR_to_PC_DataBlock: u8 = 0x80;
pub const RDR_to_PC_SlotStatus: u8 = 0x81;
pub const RDR_to_PC_Parameters: u8 = 0x82;
pub const RDR_to_PC_Escape: u8 = 0x83;
pub const RDR_to_PC_DataRateAndClockFrequency: u8 = 0x84;

// CCID response message error constants
pub const CMD_ABORTED: u8 = 0xFF;
pub const ICC_MUTE: u8 = 0xFE;
pub const XFR_PARITY_ERROR: u8 = 0xFD;
pub const XFR_OVERRUN: u8 = 0xFC;
pub const HW_ERROR: u8 = 0xFB;
pub const BAD_ATR_TS: u8 = 0xF8;
pub const BAD_ATR_TCK: u8 = 0xF7;
pub const ICC_PROTOCOL_NOT_SUPPORTED: u8 = 0xF6;
pub const ICC_CLASS_NOT_SUPPORTED: u8 = 0xF5;
pub const PROCEDURE_BYTE_CONFLICT: u8 = 0xF4;
pub const DEACTIVATED_PROTOCOL: u8 = 0xF3;
pub const BUSY_WITH_AUTO_SEQUENCE: u8 = 0xF2;
pub const PIN_TIMEOUT: u8 = 0xF0;
pub const PIN_CANCELLED: u8 = 0xEF;
pub const CMD_SLOT_BUSY: u8 = 0xE0;
pub const CMD_NOT_SUPPORTED: u8 = 0x00;
