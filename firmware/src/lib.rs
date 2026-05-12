#![no_std]
#![feature(int_roundings)]

use defmt::Format;
use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex, pubsub::PubSubChannel, watch::Watch,
};
use esp_hal::ram;
use num_enum::{IntoPrimitive, TryFromPrimitive};

use crate::{
    leds::LedCommand,
    measurements::{MeasurementResult, sampling::MEAS_SIZE, voc::STATE_SIZE},
};
extern crate alloc;
pub mod bluetooth;
pub mod button;
pub mod data;
pub mod energy;
pub mod leds;
pub mod measurements;
pub mod storage;
pub mod tasks;

#[macro_export]
macro_rules! mk_static {
    ($t:ty,$val:expr) => {{
        static STATIC_CELL: static_cell::StaticCell<$t> = static_cell::StaticCell::new();
        #[deny(unused_attributes)]
        let x = STATIC_CELL.uninit().write($val);
        x
    }};
}
#[derive(Debug, PartialEq, Eq, IntoPrimitive, TryFromPrimitive)]
#[repr(i8)]
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

#[ram(unstable(rtc_fast, persistent))]
pub static mut VOC_ALGO_STATE: [u8; STATE_SIZE] = [0u8; STATE_SIZE];

#[ram(unstable(rtc_fast, persistent))]
pub static mut LAST_CO2_SAMPLE: u64 = 0;

#[ram(unstable(rtc_fast, persistent))]
pub static mut SGP40_MEAN: f32 = 0.0;

#[ram(unstable(rtc_fast, persistent))]
pub static mut SGP40_STD: f32 = 0.0;

#[ram(unstable(rtc_fast, persistent))]
pub static mut NEEDS_SAMPLES_WRITTEN_TO_NVS: u8 = 0;

pub const SAMPLES_PER_BUFFER: usize = 160;

pub const NVS_OFFSET: usize = 0x208000;
pub const NVS_SIZE: usize = 0x20000;
pub mod nvs_keys {
    pub const SGP40_ENABLED_KEY: &[u8] = b"SGP40_EN";
    pub const STCC4_SAMPLE_RATE_KEY: &[u8] = b"STCC4_SR";
}

pub static MEASUREMENT_SIGNAL: Watch<CriticalSectionRawMutex, MeasurementResult, 1> = Watch::new();

#[derive(Debug, Format, Clone, Copy)]
pub enum Commands {
    Reconfigure(data::Config),
    Sleep(SleepOptions),
    Led(LedCommand),
}

#[derive(Debug, Format, Clone, Copy)]
pub struct SleepOptions {
    allow_buttons: bool,
    wake_in_ms: Option<u64>,
}

pub static COMMAND_CHANNEL: PubSubChannel<CriticalSectionRawMutex, Commands, 2, 3, 1> =
    PubSubChannel::new();

pub static CONFIG_SIGNAL: Watch<CriticalSectionRawMutex, data::Config, 1> = Watch::new();
