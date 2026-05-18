use aht20::AHT20;
use async_icp20100::Icp20100;
use async_stcc4::Stcc4;
use bq27441::Bq27441;
use defmt::Format;
use embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice;
use embassy_futures::join::join3;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::mutex::Mutex;
use esp_hal::Async;
use esp_hal::i2c::master::I2c;
use sgp40::Sgp40;

use crate::storage::Nvs;

#[derive(Debug, Format, Clone, Copy)]
pub struct VocData {
    /// VOC index
    pub value: i32,
    /// Sensor needs 60 readings until readings are stable
    pub readings_until_warmup_complete: i32,
    pub error: bool,
}
impl Default for VocData {
    fn default() -> Self {
        Self {
            value: 0,
            readings_until_warmup_complete: 50,
            error: false,
        }
    }
}
#[derive(Debug, Default, Format, Clone, Copy)]
pub struct PressureData {
    /// Temperature in °C
    pub temperature: f32,
    /// Pressure in kPa
    pub pressure: f32,
    pub error: bool,
}

#[derive(Debug, Default, Format, Clone, Copy)]
pub struct TemperatureData {
    /// Temperature in °C
    pub temperature: f32,
    /// Humidity in RH %
    pub humidity: f32,
    pub error: bool,
}

#[derive(Debug, Default, Format, Clone, Copy)]
pub struct Co2Data {
    /// CO2 concentration in ppm
    pub co2: i16,
    pub error: bool,
}

#[derive(Debug, Default, Format, Clone, Copy)]
pub struct Battery {
    /// Battery voltage in mV
    pub voltage: u16,
    /// Battery SoC percentage
    pub percentage: i8,
    /// Power (in mW) pulled from battery
    pub power: i16,
    pub error: bool,
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

#[derive(Debug, Format, Clone, Copy)]
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

impl Devices<'_> {
    pub async fn init(
        i2c_bus: &'static Mutex<NoopRawMutex, I2c<'static, Async>>,
        nvs: Mutex<NoopRawMutex, Nvs>,
    ) -> Devices<'static> {
        let i2c_dev_icp = I2cDevice::new(i2c_bus);
        let i2c_dev_aht = I2cDevice::new(i2c_bus);
        let i2c_dev_sgp = I2cDevice::new(i2c_bus);
        let i2c_dev_bq27 = I2cDevice::new(i2c_bus);
        let i2c_dev_stcc = I2cDevice::new(i2c_bus);

        let sgp40 = Mutex::new(Sgp40::new(i2c_dev_sgp, 0x59, embassy_time::Delay));
        let stcc4 = Mutex::new(Stcc4::new(0x65, i2c_dev_stcc, embassy_time::Delay));

        let sensors = join3(
            Bq27441::new(i2c_dev_bq27, 0x55),
            AHT20::new(i2c_dev_aht, 0x38, embassy_time::Delay),
            Icp20100::new(0x63, i2c_dev_icp, embassy_time::Delay),
        )
        .await;
        let bq27441 = Mutex::new(sensors.0.unwrap());
        let aht20 = Mutex::new(sensors.1.unwrap());
        let icp = Mutex::new(sensors.2.unwrap());

        Devices {
            aht20,
            bq27441,
            icp,
            nvs,
            sgp40,
            stcc4,
        }
    }
}
