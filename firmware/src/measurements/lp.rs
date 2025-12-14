use core::time::Duration;

use aht20::AHT20;
use embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::mutex::Mutex;
use esp_hal::gpio;
use esp_hal::i2c::master::{self as I2C, I2c};
use esp_hal::peripherals::{GPIO20, GPIO21, I2C0, LPWR};
use esp_hal::rtc_cntl::Rtc;
use esp_hal::rtc_cntl::sleep::{RtcioWakeupSource, TimerWakeupSource, WakeupLevel};
use esp_hal::time::Rate;
use esp_hal::{Async, peripherals};
use sgp40::Sgp40;

use crate::SGP40_READINGS;
pub async fn lp_measurement<'a>(
    i2c_peripheral: I2C0<'static>,
    gp20: GPIO20<'static>,
    gp21: GPIO21<'static>,
    rtc_peripheral: LPWR<'static>,
    beginning: embassy_time::Instant,
) -> bool {
    let i2c_hal = I2c::new(
        i2c_peripheral,
        I2C::Config::default().with_frequency(Rate::from_khz(400)),
    )
    .unwrap()
    .with_sda(gp20)
    .with_scl(gp21)
    .into_async();
    let i2c_bus =
        &*crate::mk_static!(Mutex<NoopRawMutex, I2c<'static, Async>>, Mutex::new(i2c_hal));
    let i2c_dev3 = I2cDevice::new(i2c_bus);
    let i2c_dev4 = I2cDevice::new(i2c_bus);
    let mut aht20 = AHT20::new(i2c_dev4, 0x38, embassy_time::Delay)
        .await
        .unwrap();
    let mut sgp40 = Sgp40::new(i2c_dev3, 0x59, embassy_time::Delay);
    let reading = aht20.measure().await.unwrap();
    sgp40
        .measure_voc_index_with_rht(
            (reading.humidity * 1000.0) as u16,
            (reading.temperature * 1000.0) as i16,
        )
        .await
        .unwrap();
    unsafe { SGP40_READINGS += 1 };
    let mut rtc = Rtc::new(rtc_peripheral);
    let elapsed = embassy_time::Instant::now() - beginning;
    let timer =
        TimerWakeupSource::new(Duration::from_secs(1) - Duration::from_millis(elapsed.as_millis()));
    let mut pin = unsafe { peripherals::GPIO3::steal() };
    let wakeup_pins: &mut [(&mut dyn gpio::RtcPinWithResistors, WakeupLevel)] =
        &mut [(&mut pin, WakeupLevel::Low)];
    let wakeup_gpio = RtcioWakeupSource::new(wakeup_pins);
    rtc.sleep_deep(&[&timer, &wakeup_gpio])
}
