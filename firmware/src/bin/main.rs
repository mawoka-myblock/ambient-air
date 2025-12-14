#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]

use aht20::AHT20;
use async_icp20100::Icp20100;
use async_stcc4::Stcc4;
use embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice;
use embassy_executor::Spawner;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::Timer;
use esp_hal::Async;
use esp_hal::clock::CpuClock;
use esp_hal::i2c::master::{self as I2C, I2c};
use esp_hal::ledc::channel::ChannelIFace;
use esp_hal::ledc::timer::TimerIFace;
use esp_hal::ledc::{Ledc, LowSpeed, channel, timer};
use esp_hal::rtc_cntl::wakeup_cause;
use esp_hal::system::SleepSource;
use esp_hal::time::Rate;
use esp_hal::timer::timg::TimerGroup;
// use esp_hal::tsens::{self, TemperatureSensor};
use esp_radio::Controller;
use esp_radio::ble::controller::BleConnector;
use firmware::bluetooth::run;
use firmware::button::button_task;
use firmware::data::{Devices, State};
use firmware::measurements::lp::lp_measurement;
use firmware::measurements::measure;
use firmware::measurements::sampling::record_sample;
use firmware::storage::Nvs;
use firmware::{PowerState, SGP40_READINGS};
use sgp40::Sgp40;
use trouble_host::prelude::ExternalController;
use {esp_backtrace as _, esp_println as _};

extern crate alloc;
// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

#[esp_rtos::main]
async fn main(spawner: Spawner) {
    // generator version: 0.5.0

    let beginning = embassy_time::Instant::now();

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    esp_alloc::heap_allocator!(size: 64 * 1024);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    let sw_interrupt =
        esp_hal::interrupt::software::SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);
    esp_rtos::start(timg0.timer0, sw_interrupt.software_interrupt0);
    let wakeup_cause_var = wakeup_cause();
    if wakeup_cause_var as i8 != SleepSource::Timer as i8 {
        // Just checking for Ext0 or Ext1 doesn't work, nee do figure out which event is the gpio button, but cheking for not timer works fine
        unsafe { firmware::POWER_STATE = PowerState::BluetoothMode as i8 }
    }

    if unsafe { firmware::POWER_STATE == PowerState::SensorActiveSleep as i8 } {
        lp_measurement(
            peripherals.I2C0,
            peripherals.GPIO20,
            peripherals.GPIO21,
            peripherals.LPWR,
            beginning,
        )
        .await;
        unreachable!()
    }

    // I2C Block
    let i2c_hal = I2C::I2c::new(peripherals.I2C0, I2C::Config::default())
        .unwrap()
        .with_sda(peripherals.GPIO20)
        .with_scl(peripherals.GPIO21)
        .into_async();
    let i2c_bus =
        &*firmware::mk_static!(Mutex<NoopRawMutex, I2c<'static, Async>>, Mutex::new(i2c_hal));
    let i2c_dev1 = I2cDevice::new(i2c_bus);
    let icp = Icp20100::new(0x63, i2c_dev1, embassy_time::Delay)
        .await
        .unwrap();
    let i2c_dev2 = I2cDevice::new(i2c_bus);
    let stcc4 = Stcc4::new(0x65, i2c_dev2, embassy_time::Delay);
    let i2c_dev3 = I2cDevice::new(i2c_bus);
    let sgp40 = Sgp40::new(i2c_dev3, 0x59, embassy_time::Delay);
    let i2c_dev4 = I2cDevice::new(i2c_bus);
    let aht20 = AHT20::new(i2c_dev4, 0x38, embassy_time::Delay)
        .await
        .unwrap();
    let devices = Devices {
        icp,
        stcc4,
        sgp40,
        aht20,
    };

    // END I2C Blocks

    if unsafe { firmware::POWER_STATE == PowerState::SampleMode as i8 } {
        // unsafe {
        //     firmware::MEASUREMENT_SAMPLES_TAKEN = 0;
        //     firmware::MEASUREMENT_SAMPLES_REQUESTED = 1;
        //     firmware::SAMPLE_EVERY_SECONDS = 2;
        //     firmware::POWER_STATE = PowerState::SampleMode as i8;
        // }
        let mut nvs =
            Nvs::new(firmware::NVS_OFFSET, firmware::NVS_SIZE, peripherals.FLASH).unwrap();
        // nvs.append_key(b"test", b"buf").await.unwrap();
        record_sample(devices, beginning, &mut nvs).await;
        unreachable!();
    }

    // LEDC Block
    let mut ledc = Ledc::new(peripherals.LEDC);
    ledc.set_global_slow_clock(esp_hal::ledc::LSGlobalClkSource::APBClk);
    let mut lstimer0 = ledc.timer::<LowSpeed>(timer::Number::Timer2);
    lstimer0
        .configure(timer::config::Config {
            duty: timer::config::Duty::Duty10Bit,
            clock_source: timer::LSClockSource::APBClk,
            frequency: Rate::from_khz(24),
        })
        .unwrap();
    let mut channel0 = ledc.channel::<LowSpeed>(channel::Number::Channel0, peripherals.GPIO5);
    channel0
        .configure(channel::config::Config {
            timer: &lstimer0,
            duty_pct: 0,
            drive_mode: esp_hal::gpio::DriveMode::PushPull,
        })
        .unwrap();
    let mut channel1 = ledc.channel::<LowSpeed>(channel::Number::Channel2, peripherals.GPIO10);
    channel1
        .configure(channel::config::Config {
            timer: &lstimer0,
            duty_pct: 0,
            drive_mode: esp_hal::gpio::DriveMode::PushPull,
        })
        .unwrap();
    // END LEDC Block

    // let internal_temp_sensor =
    //     TemperatureSensor::new(peripherals.TSENS, tsens::Config::default()).unwrap();

    let state: &'static State = &*firmware::mk_static!(State, State::default());
    {
        let mut mut_voc = state.voc.lock().await;
        mut_voc.readings_until_warmup_complete = mut_voc
            .readings_until_warmup_complete
            .saturating_sub(unsafe { SGP40_READINGS } as i32)
            .clamp(0, 50);
    }
    spawner.spawn(measure(state, devices)).unwrap();
    spawner.spawn(button_task(state)).unwrap();

    // Bluetooth Block
    let radio: &'static Controller<'_> =
        &*firmware::mk_static!(Controller, esp_radio::init().unwrap());
    let bluetooth = peripherals.BT;
    let connector = BleConnector::new(&radio, bluetooth, Default::default()).unwrap();
    let controller: ExternalController<_, 20> = ExternalController::new(connector);
    // END Bluetooth Block

    spawner.spawn(run(controller, state)).unwrap();
    // info!("Test key: {}", str::from_utf8(&test_key).unwrap());

    loop {
        // let raw_data: u16 = adc1.read_oneshot(&mut pin).await;
        // let raw_voltage = raw_data as u32 * 2500 / 4095;
        // let bat_voltage: f32 = raw_voltage as f32 * 2.2 / 1000.0; // voltage in volts
        channel1.start_duty_fade(0, 15, 2000).unwrap();
        Timer::after_millis(2000).await;
        channel1.start_duty_fade(15, 0, 2000).unwrap();
        Timer::after_millis(2000).await;
        // info!("Is button pressed: {}", input_btn.is_high());
        // Timer::after_millis(500).await;
    }

    // for inspiration have a look at the examples at https://github.com/esp-rs/esp-hal/tree/esp-hal-v1.0.0-rc.0/examples/src/bin
}
