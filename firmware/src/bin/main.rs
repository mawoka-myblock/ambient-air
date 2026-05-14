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
use bq27441::Bq27441;
use embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice;
use embassy_executor::Spawner;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::Timer;
use esp_hal::Async;
use esp_hal::clock::CpuClock;
use esp_hal::i2c::master::{self as I2C, I2c};
use esp_hal::ledc::timer::TimerIFace;
use esp_hal::ledc::{Ledc, LowSpeed, timer};
use esp_hal::rtc_cntl::wakeup_cause;
use esp_hal::system::SleepSource;
use esp_hal::time::Rate;
use esp_hal::timer::timg::TimerGroup;
// use esp_hal::tsens::{self, TemperatureSensor};
use esp_radio::ble::controller::BleConnector;
use firmware::bluetooth::run;
use firmware::button::button_task;
use firmware::data::Devices;
use firmware::energy::set_sgp40;
use firmware::leds::{Leds, led_task};
use firmware::measurements::lp::lp_measurement;
use firmware::measurements::measure;
use firmware::measurements::sampling::{move_to_nvs, record_sample};
use firmware::measurements::voc::restore_voc_state;
use firmware::storage::Nvs;
use firmware::tasks::{settings, sleep};
use firmware::{COMMAND_CHANNEL, Commands, PowerState};
use sgp40::Sgp40;
use trouble_host::prelude::ExternalController;
use {esp_backtrace as _, esp_println as _};

extern crate alloc;
esp_bootloader_esp_idf::esp_app_desc!();

#[esp_rtos::main]
async fn main(spawner: Spawner) {
    let wakeup_cause_var = wakeup_cause();
    if wakeup_cause_var as i8 == SleepSource::Gpio as i8 {
        unsafe { firmware::POWER_STATE = PowerState::BluetoothMode as i8 }
    }

    // Set CPU frequency to try to save as much power as possible
    let config = esp_hal::Config::default().with_cpu_clock(
        match PowerState::try_from(unsafe { firmware::POWER_STATE })
            .unwrap_or(PowerState::SampleMode)
        {
            PowerState::DeepSleep => CpuClock::_80MHz,
            PowerState::BluetoothMode => CpuClock::_160MHz,
            PowerState::SampleMode => CpuClock::_80MHz, // _80MHz
            PowerState::SensorActiveSleep => CpuClock::_80MHz,
        },
    );

    // -----------------
    // Init peripherals and rtos
    // -----------------
    let peripherals = esp_hal::init(config);
    esp_alloc::heap_allocator!(size: 64 * 1024);
    let timg0 = TimerGroup::new(peripherals.TIMG0);
    let sw_interrupt =
        esp_hal::interrupt::software::SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);
    esp_rtos::start(timg0.timer0, sw_interrupt.software_interrupt0);
    let beginning = embassy_time::Instant::now();

    // -----------------
    // Run LP measurement
    // -----------------
    if unsafe { firmware::POWER_STATE == PowerState::SensorActiveSleep as i8 } {
        lp_measurement(
            peripherals.I2C0,
            peripherals.GPIO20,
            peripherals.GPIO21,
            peripherals.LPWR,
        )
        .await;
    }

    // -----------------
    // Init I2C
    // -----------------
    let i2c_hal = I2C::I2c::new(
        peripherals.I2C0,
        I2C::Config::default().with_frequency(Rate::from_khz(100)),
    )
    .unwrap()
    .with_sda(peripherals.GPIO20)
    .with_scl(peripherals.GPIO21)
    .into_async();
    let i2c_bus =
        &*firmware::mk_static!(Mutex<NoopRawMutex, I2c<'static, Async>>, Mutex::new(i2c_hal));

    let i2c_dev_stcc = I2cDevice::new(i2c_bus);
    let stcc4 = Mutex::new(Stcc4::new(0x65, i2c_dev_stcc, embassy_time::Delay));

    let i2c_dev_icp = I2cDevice::new(i2c_bus);
    let icp = Mutex::new(
        Icp20100::new(0x63, i2c_dev_icp, embassy_time::Delay)
            .await
            .unwrap(),
    );

    let i2c_dev_aht = I2cDevice::new(i2c_bus);
    let aht20 = Mutex::new(
        AHT20::new(i2c_dev_aht, 0x38, embassy_time::Delay)
            .await
            .unwrap(),
    );

    let i2c_dev_sgp = I2cDevice::new(i2c_bus);
    let sgp40 = Mutex::new(Sgp40::new(i2c_dev_sgp, 0x59, embassy_time::Delay));

    let i2c_dev_bq27 = I2cDevice::new(i2c_bus);
    let bq27441 = Mutex::new(Bq27441::new(i2c_dev_bq27, 0x55).await.unwrap());

    // -----------------
    // Restore SGP40 algo data
    // -----------------
    let restored_voc_state = restore_voc_state();
    if restored_voc_state.uptime > 0.0 {
        sgp40.lock().await.set_algorithm_state(&restored_voc_state);
    }

    // -----------------
    // Init the NVS storage (tickv)
    // -----------------
    let raw_nvs = Nvs::new(firmware::NVS_OFFSET, firmware::NVS_SIZE, peripherals.FLASH).unwrap();
    let nvs = Mutex::new(raw_nvs);

    // -----------------
    // Init devices struct in &'static
    // -----------------
    let devices: &'static Devices = firmware::mk_static!(
        Devices,
        Devices {
            icp,
            stcc4,
            sgp40,
            aht20,
            bq27441,
            nvs
        }
    );

    // -----------------
    // Run sample mode fn
    // -----------------
    if unsafe { firmware::POWER_STATE == PowerState::SampleMode as i8 } {
        record_sample(devices, beginning, &mut *devices.nvs.lock().await).await;
    }

    set_sgp40(devices).await;

    // -----------------
    // Save sample if that is needed
    // -----------------
    if unsafe { firmware::NEEDS_SAMPLES_WRITTEN_TO_NVS == 1 } {
        let mut nvs_l = devices.nvs.lock().await;
        move_to_nvs(&mut nvs_l).await;
        unsafe { firmware::NEEDS_SAMPLES_WRITTEN_TO_NVS = 0 };
    }

    // -----------------
    // Init LED peripherals and struct
    // -----------------
    let ledc = firmware::mk_static!(Ledc<'static>, Ledc::new(peripherals.LEDC));
    ledc.set_global_slow_clock(esp_hal::ledc::LSGlobalClkSource::APBClk);
    let lstimer0: &'static mut timer::Timer<'static, LowSpeed> = firmware::mk_static!(
        timer::Timer<'static, LowSpeed>,
        ledc.timer::<LowSpeed>(timer::Number::Timer2)
    );
    lstimer0
        .configure(timer::config::Config {
            duty: timer::config::Duty::Duty10Bit,
            clock_source: timer::LSClockSource::APBClk,
            frequency: Rate::from_khz(24),
        })
        .unwrap();
    let leds: &'static mut Leds<'static> = firmware::mk_static!(
        Leds<'static>,
        Leds::new(ledc, lstimer0, peripherals.GPIO5, peripherals.GPIO10)
    );
    // -----------------
    // Spawn main tasks
    // -----------------
    spawner.spawn(led_task(leds).unwrap());
    spawner.spawn(measure(devices).unwrap());
    spawner.spawn(button_task(peripherals.GPIO3, peripherals.GPIO4).unwrap());
    spawner.spawn(sleep::sleep_task(devices, peripherals.LPWR).unwrap());
    spawner.spawn(settings::settings_task().unwrap());

    // -----------------
    // Init Bluetooth
    // -----------------
    let connector = BleConnector::new(peripherals.BT, Default::default()).unwrap();
    let controller: ExternalController<_, 20> = ExternalController::new(connector);

    spawner.spawn(run(controller, devices).unwrap());

    // -----------------
    // Main loop breathing led
    // -----------------
    firmware::tasks::battery::show_battery_percentage().await;

    loop {
        Timer::after_millis(1500).await;
        COMMAND_CHANNEL
            .immediate_publisher()
            .publish_immediate(Commands::Led(firmware::leds::LedCommand::Set {
                led: 1,
                level: 100,
            }));
        Timer::after_millis(20).await;
        COMMAND_CHANNEL
            .immediate_publisher()
            .publish_immediate(Commands::Led(firmware::leds::LedCommand::Set {
                led: 1,
                level: 0,
            }));
    }
}
