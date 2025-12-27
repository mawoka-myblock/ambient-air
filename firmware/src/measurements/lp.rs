use core::time::Duration;

use aht20::AHT20;
use async_icp20100::Icp20100;
use async_stcc4::Stcc4;
use embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice;
use embassy_futures::join::join;
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
pub async fn lp_measurement(
    i2c_peripheral: I2C0<'static>,
    gp20: GPIO20<'static>,
    gp21: GPIO21<'static>,
    rtc_peripheral: LPWR<'static>,
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
    let i2c_dev1 = I2cDevice::new(i2c_bus);
    let i2c_dev2 = I2cDevice::new(i2c_bus);
    let i2c_dev3 = I2cDevice::new(i2c_bus);
    let aht20_fut = AHT20::new(i2c_dev1, 0x38, embassy_time::Delay);
    let icp_fut = Icp20100::new(0x63, i2c_dev3, embassy_time::Delay);
    let (mut aht20, mut icp) = {
        let (aht_res, icp_res) = join(aht20_fut, icp_fut).await;
        (aht_res.unwrap(), icp_res.unwrap())
    };
    let mut stcc4 = Stcc4::new(0x59, i2c_dev2, embassy_time::Delay);
    let (reading, pressure) = {
        let (read_res, pres_res) = join(aht20.measure(), icp.read_pressure()).await;
        (read_res.unwrap(), pres_res.unwrap())
    };

    let _ = stcc4
        .set_pressure_compensation((pressure * 1000.0) as i32)
        .await;
    let _ = stcc4
        .set_rht_compensation(reading.temperature, reading.humidity)
        .await;
    let _ = stcc4.single_shot(false).await;
    let mut sleep_dur = Duration::from_secs(unsafe { crate::STCC4_SAMPLE_RATE } as u64);

    if unsafe { crate::SGP40_ENABLED == 1 } {
        let i2c_dev4 = I2cDevice::new(i2c_bus);
        let mut sgp40 = Sgp40::new(i2c_dev4, 0x59, embassy_time::Delay);
        sgp40
            .measure_voc_index_with_rht(
                (reading.humidity * 1000.0) as u16,
                (reading.temperature * 1000.0) as i16,
            )
            .await
            .unwrap();
        sleep_dur = Duration::from_secs(1);
        unsafe { SGP40_READINGS += 1 };
    }

    let mut rtc = Rtc::new(rtc_peripheral);
    let timer = TimerWakeupSource::new(sleep_dur);
    let mut pin = unsafe { peripherals::GPIO3::steal() };
    let wakeup_pins: &mut [(&mut dyn gpio::RtcPinWithResistors, WakeupLevel)] =
        &mut [(&mut pin, WakeupLevel::Low)];
    let wakeup_gpio = RtcioWakeupSource::new(wakeup_pins);
    rtc.sleep_deep(&[&timer, &wakeup_gpio])
}
