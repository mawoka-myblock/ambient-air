use aht20::AHT20;
use async_icp20100::Icp20100;
use async_stcc4::Stcc4;
use bq27441::Bq27441;
use defmt::Format;
use embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::mutex::Mutex;
use esp_hal::Async;
use esp_hal::i2c::master::I2c;
use sgp40::Sgp40;

use crate::storage::Nvs;

#[derive(Debug, Format)]
pub struct VocData {
    /// VOC index
    pub value: i32,
    /// Sensor needs 60 readings until readings are stable
    pub readings_until_warmup_complete: i32,
}
impl Default for VocData {
    fn default() -> Self {
        Self {
            value: 0,
            readings_until_warmup_complete: 50,
        }
    }
}
#[derive(Debug, Default, Format)]
pub struct PressureData {
    /// Temperature in °C
    pub temperature: f32,
    /// Pressure in kPa
    pub pressure: f32,
}

#[derive(Debug, Default, Format)]
pub struct TemperatureData {
    /// Temperature in °C
    pub temperature: f32,
    /// Humidity in RH %
    pub humidity: f32,
}

#[derive(Debug, Default, Format)]
pub struct Co2Data {
    /// CO2 concentration in ppm
    pub co2: i16,
}

#[derive(Debug, Default, Format)]
pub struct Battery {
    /// Battery voltage in mV
    pub voltage: u16,
    /// Battery SoC percentage
    pub percentage: i8,
    /// Power (in mW) pulled from battery
    pub power: i16,
}

#[derive(Debug, Default)]
pub struct State {
    pub voc: Mutex<NoopRawMutex, VocData>,
    pub pressure: Mutex<NoopRawMutex, PressureData>,
    pub temperature: Mutex<NoopRawMutex, TemperatureData>,
    pub co2: Mutex<NoopRawMutex, Co2Data>,
    pub battery: Mutex<NoopRawMutex, Battery>,
    pub config: Mutex<NoopRawMutex, Config>,
}

#[derive(Debug, Format)]
pub struct Config {
    pub update_interval: i32,
}
impl Default for Config {
    fn default() -> Self {
        Self { update_interval: 1 }
    }
}
pub type ShortI2cDevice<'a> = I2cDevice<'a, NoopRawMutex, I2c<'a, Async>>;
pub struct Devices<'a> {
    pub icp: Mutex<NoopRawMutex, Icp20100<ShortI2cDevice<'a>, embassy_time::Delay>>,
    pub stcc4: Mutex<NoopRawMutex, Stcc4<ShortI2cDevice<'a>, embassy_time::Delay>>,
    pub sgp40: Mutex<NoopRawMutex, Sgp40<ShortI2cDevice<'a>, embassy_time::Delay>>,
    pub aht20: Mutex<NoopRawMutex, AHT20<ShortI2cDevice<'a>, embassy_time::Delay>>,
    pub bq27441: Mutex<NoopRawMutex, Bq27441<ShortI2cDevice<'a>>>,
    pub nvs: Mutex<NoopRawMutex, Nvs>,
}
