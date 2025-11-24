#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]

use core::cell::RefCell;

use aht20::AHT20;
use async_icp20100::Icp20100;
use async_stcc4::Stcc4;
use bt_hci::controller::ExternalController;
use defmt::{error, info};
use embassy_executor::Spawner;
use embassy_time::{Delay, Duration, Timer};
use embedded_hal_bus::i2c;
use esp_hal::analog::adc::{Adc, AdcCalBasic, AdcCalCurve, AdcCalLine, AdcConfig};
use esp_hal::i2c::master as I2C;
use esp_hal::peripherals::ADC1;
use esp_hal::timer::systimer::SystemTimer;
use esp_hal::timer::timg::TimerGroup;
use esp_hal::{analog::adc::Attenuation, clock::CpuClock};
use esp_wifi::ble::controller::BleConnector;
use sgp40::Sgp40;
use {esp_backtrace as _, esp_println as _};

extern crate alloc;

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

#[esp_hal_embassy::main]
async fn main(spawner: Spawner) {
    // generator version: 0.5.0

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    esp_alloc::heap_allocator!(size: 64 * 1024);

    let timer0 = SystemTimer::new(peripherals.SYSTIMER);
    esp_hal_embassy::init(timer0.alarm0);

    info!("Embassy initialized!");

    let rng = esp_hal::rng::Rng::new(peripherals.RNG);
    let timer1 = TimerGroup::new(peripherals.TIMG0);
    // let wifi_init =
    // esp_wifi::init(timer1.timer0, rng).expect("Failed to initialize WIFI/BLE controller");
    // find more examples https://github.com/embassy-rs/trouble/tree/main/examples/esp32
    // let transport = BleConnector::new(&wifi_init, peripherals.BT);
    // let _ble_controller = ExternalController::<_, 20>::new(transport);

    // TODO: Spawn some tasks
    let _ = spawner;

    let mut adc1_config: AdcConfig<ADC1> = AdcConfig::new();
    let mut pin =
        adc1_config.enable_pin_with_cal::<_, AdcCalBasic<_>>(peripherals.GPIO2, Attenuation::_11dB);
    let mut adc1 = Adc::new(peripherals.ADC1, adc1_config).into_async();

    let i2c_hal = I2C::I2c::new(peripherals.I2C0, I2C::Config::default())
        .unwrap()
        .with_sda(peripherals.GPIO20)
        .with_scl(peripherals.GPIO21);
    let i2c_ref_cell = RefCell::new(i2c_hal);
    let mut icp = Icp20100::new(
        0x63,
        i2c::RefCellDevice::new(&i2c_ref_cell),
        embassy_time::Delay,
    )
    .await
    .unwrap();
    let mut stcc4 = Stcc4::new(
        0x65,
        i2c::RefCellDevice::new(&i2c_ref_cell),
        embassy_time::Delay,
    );
    let mut sgp40 = Sgp40::new(
        i2c::RefCellDevice::new(&i2c_ref_cell),
        0x59,
        embassy_time::Delay,
    );
    let mut aht20 = AHT20::new(
        i2c::RefCellDevice::new(&i2c_ref_cell),
        0x38,
        embassy_time::Delay,
    )
    .await
    .unwrap();

    loop {
        // let raw_data: u16 = adc1.read_oneshot(&mut pin).await;
        // let raw_voltage = raw_data as u32 * 2500 / 4095;
        // let bat_voltage: f32 = raw_voltage as f32 * 2.2 / 1000.0; // voltage in volts
        let data = icp.read_pressure_and_temperature().unwrap();
        info!("ICP: data: {:?}", data);
        stcc4.single_shot().await.unwrap();
        let (co2, t, rh) = stcc4.read_measurement().await.unwrap();
        info!("STCC4: CO2: {}, t: {}, rh: {}", co2, t, rh);
        let d = sgp40
            .measure_voc_index_with_rht((rh * 1000.0) as u16, (t * 1000.0) as i16)
            .unwrap();
        info!("SGP40: {}", d);
        let aht20_data = aht20.measure().await.unwrap();
        info!(
            "AHT20 data: Temp: {:?}, humidity: {}",
            aht20_data.temperature, aht20_data.humidity
        );
        Timer::after(Duration::from_secs(1)).await;
    }

    // for inspiration have a look at the examples at https://github.com/esp-rs/esp-hal/tree/esp-hal-v1.0.0-rc.0/examples/src/bin
}
