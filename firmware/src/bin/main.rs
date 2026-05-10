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
use defmt::{Debug2Format, Display2Format, info};
use embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice;
use embassy_executor::Spawner;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::Timer;
use esp_bootloader_esp_idf::partitions::{PartitionType, read_partition_table};
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
use firmware::energy::set_sgp40;
use firmware::leds::{FadeConfig, Leds, led_task};
use firmware::measurements::lp::lp_measurement;
use firmware::measurements::measure;
use firmware::measurements::sampling::{move_to_nvs, record_sample};
use firmware::measurements::voc::{restore_voc_state, store_voc_state};
use firmware::storage::Nvs;
use firmware::{PowerState, SGP40_READINGS};
use sgp40::{Sgp40, VocAlgorithmState};
use trouble_host::prelude::ExternalController;
use {esp_backtrace as _, esp_println as _};

extern crate alloc;
esp_bootloader_esp_idf::esp_app_desc!();

#[esp_rtos::main]
async fn main(spawner: Spawner) {
    let beginning = embassy_time::Instant::now();
    let wakeup_cause_var = wakeup_cause();
    if wakeup_cause_var as i8 == SleepSource::Gpio as i8 {
        unsafe { firmware::POWER_STATE = PowerState::BluetoothMode as i8 }
    }

    let config = esp_hal::Config::default().with_cpu_clock(
        match PowerState::try_from(unsafe { firmware::POWER_STATE })
            .unwrap_or(PowerState::SampleMode)
        {
            PowerState::DeepSleep => CpuClock::_80MHz,
            PowerState::BluetoothMode => CpuClock::_160MHz,
            PowerState::SampleMode => CpuClock::_80MHz,
            PowerState::SensorActiveSleep => CpuClock::_80MHz,
        },
    );
    let peripherals = esp_hal::init(config);

    esp_alloc::heap_allocator!(size: 64 * 1024);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    let sw_interrupt =
        esp_hal::interrupt::software::SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);
    esp_rtos::start(timg0.timer0, sw_interrupt.software_interrupt0);

    if unsafe { firmware::POWER_STATE == PowerState::SensorActiveSleep as i8 } {
        lp_measurement(
            peripherals.I2C0,
            peripherals.GPIO20,
            peripherals.GPIO21,
            peripherals.LPWR,
        )
        .await;
    }

    // I2C Block
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

    let i2c_dev2 = I2cDevice::new(i2c_bus);
    let stcc4 = Mutex::new(Stcc4::new(0x65, i2c_dev2, embassy_time::Delay));

    let i2c_dev1 = I2cDevice::new(i2c_bus);
    let icp = Mutex::new(
        Icp20100::new(0x63, i2c_dev1, embassy_time::Delay)
            .await
            .unwrap(),
    );

    let i2c_dev4 = I2cDevice::new(i2c_bus);
    let aht20 = Mutex::new(
        AHT20::new(i2c_dev4, 0x38, embassy_time::Delay)
            .await
            .unwrap(),
    );

    let i2c_dev3 = I2cDevice::new(i2c_bus);
    let sgp40 = Mutex::new(Sgp40::new(i2c_dev3, 0x59, embassy_time::Delay));

    let restored_voc_state = restore_voc_state();
    info!("Restored data: {}", Debug2Format(&restored_voc_state));
    if restored_voc_state.uptime > 0.0 {
        sgp40.lock().await.set_algorithm_state(&restored_voc_state);
    }

    let i2c_dev5 = I2cDevice::new(i2c_bus);
    let bq27441 = Mutex::new(Bq27441::new(i2c_dev5, 0x55).await.unwrap());
    // let mut flash = esp_storage::FlashStorage::new(peripherals.FLASH);

    // let mut buffer = [0u8; esp_bootloader_esp_idf::partitions::PARTITION_TABLE_MAX_LEN];
    // let ptns = read_partition_table(&mut flash, &mut buffer).unwrap();
    // for i in 0..ptns.len() {
    //     let pt = ptns.get_partition(i).unwrap();
    //     info!(
    //         "{:?}, offset: 0x{:x} len: 0x{:x}",
    //         pt,
    //         pt.offset(),
    //         pt.len()
    //     );
    // }
    // return;

    let raw_nvs = Nvs::new(firmware::NVS_OFFSET, firmware::NVS_SIZE, peripherals.FLASH).unwrap();

    // firmware::measurements::sampling::move_to_nvs(&mut raw_nvs, 3).await;

    let nvs = Mutex::new(raw_nvs);

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

    // END I2C Blocks

    if unsafe { firmware::POWER_STATE == PowerState::SampleMode as i8 } {
        record_sample(devices, beginning, &mut *devices.nvs.lock().await).await;
        unreachable!();
    }

    set_sgp40(devices).await;

    if unsafe { firmware::NEEDS_SAMPLES_WRITTEN_TO_NVS == 1 } {
        info!("Saving to NVS!");
        let mut nvs_l = devices.nvs.lock().await;
        move_to_nvs(&mut nvs_l).await;
        unsafe { firmware::NEEDS_SAMPLES_WRITTEN_TO_NVS = 0 };
    }

    // LEDC Block
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
    let led_channel: &'static firmware::leds::LedChannel = firmware::mk_static!(
        firmware::leds::LedChannel,
        firmware::leds::LedChannel::new()
    );
    spawner.spawn(led_task(led_channel, leds)).unwrap();
    // for _ in 0..wakeup_cause_var as u8 {
    //     led_channel
    //         .send(firmware::leds::LedCommand::Set { led: 1, level: 100 })
    //         .await;
    //     Timer::after_millis(500).await;
    //     led_channel
    //         .send(firmware::leds::LedCommand::Set { led: 1, level: 0 })
    //         .await;
    //     Timer::after_millis(500).await;
    // }

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
    let connector = BleConnector::new(radio, bluetooth, Default::default()).unwrap();
    let controller: ExternalController<_, 20> = ExternalController::new(connector);
    // END Bluetooth Block

    spawner.spawn(run(controller, state, devices)).unwrap();
    loop {
        led_channel
            .send(firmware::leds::LedCommand::Fade((
                FadeConfig {
                    start_pct: 0,
                    end_pct: 15,
                    fade_dur: 2000,
                },
                2,
            )))
            .await;
        Timer::after_millis(2000).await;
        led_channel
            .send(firmware::leds::LedCommand::Fade((
                FadeConfig {
                    start_pct: 15,
                    end_pct: 0,
                    fade_dur: 2000,
                },
                2,
            )))
            .await;
        Timer::after_millis(2000).await;
    }
}
