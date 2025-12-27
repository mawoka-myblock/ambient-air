#![allow(dead_code)]
pub const DEFAULT_ADDR: u8 = 0x55;
pub const DEVICE_ID: u16 = 0x0421;
pub const UNSEAL_KEY: u16 = 0x8000;

// Standard commands (2 bytes, little-endian)
pub const CMD_CONTROL: u8 = 0x00;
pub const CMD_TEMP: u8 = 0x02;
pub const CMD_VOLTAGE: u8 = 0x04;
pub const CMD_FLAGS: u8 = 0x06;
pub const CMD_REM_CAPACITY: u8 = 0x0C;
pub const CMD_FULL_CAPACITY: u8 = 0x0E;
pub const CMD_AVG_CURRENT: u8 = 0x10;
pub const CMD_AVG_POWER: u8 = 0x18;
pub const CMD_SOC: u8 = 0x1C;
pub const CMD_INT_TEMP: u8 = 0x1E;
pub const CMD_SOH: u8 = 0x20;

// Control subcommands
pub const CTRL_STATUS: u16 = 0x0000;
pub const CTRL_DEVICE_TYPE: u16 = 0x0001;
pub const CTRL_SET_CFGUPDATE: u16 = 0x0013;
pub const CTRL_EXIT_CFGUPDATE: u16 = 0x0043;
pub const CTRL_SOFT_RESET: u16 = 0x0042;
pub const CTRL_SEALED: u16 = 0x0020;

// Extended data
pub const EXT_OPCONFIG: u8 = 0x3A;
pub const EXT_CAPACITY: u8 = 0x3C;
pub const EXT_DATACLASS: u8 = 0x3E;
pub const EXT_DATABLOCK: u8 = 0x3F;
pub const EXT_BLOCKDATA: u8 = 0x40;
pub const EXT_CHECKSUM: u8 = 0x60;
pub const EXT_CONTROL: u8 = 0x61;
