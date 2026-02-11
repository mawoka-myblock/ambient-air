#![no_std]

use esp_hal::ram;

use crate::measurements::sampling::MEAS_SIZE;
extern crate alloc;
pub mod bluetooth;
pub mod button;
pub mod data;
pub mod energy;
pub mod leds;
pub mod measurements;
pub mod storage;

#[macro_export]
macro_rules! mk_static {
    ($t:ty,$val:expr) => {{
        static STATIC_CELL: static_cell::StaticCell<$t> = static_cell::StaticCell::new();
        #[deny(unused_attributes)]
        let x = STATIC_CELL.uninit().write($val);
        x
    }};
}
#[derive(Debug, PartialEq, Eq)]
pub enum PowerState {
    DeepSleep = 0,
    SensorActiveSleep = 1,
    BluetoothMode = 2,
    SampleMode = 3,
}
/// Only use with PowerState enum
#[ram(unstable(rtc_fast, persistent))]
pub static mut POWER_STATE: i8 = 0;

/// Must only be 0 or 1, as bool isn't allowed here
#[ram(unstable(rtc_fast, persistent))]
pub static mut SGP40_ENABLED: u8 = 0;

#[ram(unstable(rtc_fast, persistent))]
pub static mut STCC4_SAMPLE_RATE: i16 = 600;

#[ram(unstable(rtc_fast, persistent))]
pub static mut SGP40_READINGS: i16 = 0;

#[ram(unstable(rtc_fast, persistent))]
pub static mut MEASUREMENT_SAMPLES_REQUESTED: i16 = 0;

#[ram(unstable(rtc_fast, persistent))]
pub static mut MEASUREMENT_SAMPLES_TAKEN: i16 = 0;

#[ram(unstable(rtc_fast, persistent))]
pub static mut SAMPLE_EVERY_SECONDS: i16 = 0;

#[ram(unstable(rtc_fast, persistent))]
pub static mut SAMPLE_BUFFER: [u8; SAMPLES_PER_BUFFER * MEAS_SIZE] =
    [0u8; SAMPLES_PER_BUFFER * MEAS_SIZE]; // 160 samples in these 3520 bytes (22 bytes/sample)

pub const SAMPLES_PER_BUFFER: usize = 160;

pub const NVS_OFFSET: usize = 0x9000;
pub const NVS_SIZE: usize = 0x6000;
pub mod nvs_keys {
    pub const SGP40_ENABLED_KEY: &[u8] = b"SGP40_EN";
    pub const STCC4_SAMPLE_RATE_KEY: &[u8] = b"STCC4_SR";
}
