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
use defmt::info;
use embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice;
use embassy_executor::Spawner;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::{Duration, Timer};
use esp_hal::Async;
use esp_hal::analog::adc::{Adc, AdcCalBasic, AdcConfig};
use esp_hal::gpio::{Input, InputConfig};
use esp_hal::i2c::master::{self as I2C, I2c};
use esp_hal::ledc::channel::ChannelIFace;
use esp_hal::ledc::timer::TimerIFace;
use esp_hal::ledc::{Ledc, LowSpeed, channel, timer};
use esp_hal::peripherals::ADC1;
use esp_hal::rtc_cntl::Rtc;
use esp_hal::time::Rate;
use esp_hal::timer::systimer::SystemTimer;
use esp_hal::timer::timg::TimerGroup;
use esp_hal::tsens::{self, TemperatureSensor};
use esp_hal::{analog::adc::Attenuation, clock::CpuClock};
use esp_radio::Controller;
use esp_radio::ble::controller::BleConnector;
use firmware::bluetooth::run;
use firmware::button::{ButtonDevices, button_task};
use firmware::data::{Devices, State};
use firmware::measurements::measure;
use sgp40::Sgp40;
use static_cell::StaticCell;
use trouble_host::prelude::ExternalController;
use {esp_backtrace as _, esp_println as _};

extern crate alloc;
// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

static I2C_BUS: StaticCell<Mutex<NoopRawMutex, I2c<'static, Async>>> = StaticCell::new();
static STATE: StaticCell<State> = StaticCell::new();
static RADIO: StaticCell<Controller> = StaticCell::new();
static BUTTON_DEVICES: StaticCell<ButtonDevices> = StaticCell::new();
static INPUT_BUTTON: StaticCell<Input> = StaticCell::new();
static RTC_CLOCK: StaticCell<Rtc> = StaticCell::new();

#[esp_rtos::main]
async fn main(spawner: Spawner) {
    // generator version: 0.5.0

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    esp_alloc::heap_allocator!(size: 64 * 1024);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    let sw_interrupt =
        esp_hal::interrupt::software::SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);
    esp_rtos::start(timg0.timer0, sw_interrupt.software_interrupt0);

    info!("Embassy initialized!");

    // TODO: Spawn some tasks

    let mut adc1_config: AdcConfig<ADC1> = AdcConfig::new();
    let mut pin =
        adc1_config.enable_pin_with_cal::<_, AdcCalBasic<_>>(peripherals.GPIO2, Attenuation::_11dB);
    let mut adc1 = Adc::new(peripherals.ADC1, adc1_config).into_async();

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

    let internal_temp_sensor =
        TemperatureSensor::new(peripherals.TSENS, tsens::Config::default()).unwrap();

    let input_btn: &'static mut Input = INPUT_BUTTON.init(Input::new(
        peripherals.GPIO3,
        InputConfig::default().with_pull(esp_hal::gpio::Pull::Up),
    ));
    input_btn
        .wakeup_enable(true, esp_hal::gpio::WakeEvent::LowLevel)
        .unwrap();
    let rtc: &'static mut Rtc = RTC_CLOCK.init(Rtc::new(peripherals.LPWR));

    let i2c_hal = I2C::I2c::new(peripherals.I2C0, I2C::Config::default())
        .unwrap()
        .with_sda(peripherals.GPIO20)
        .with_scl(peripherals.GPIO21)
        .into_async();
    let i2c_bus = Mutex::new(i2c_hal);
    let i2c_bus = I2C_BUS.init(i2c_bus);
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
        adc: adc1,
    };
    let state: &'static State = STATE.init(State::default());
    spawner.spawn(measure(state, devices)).unwrap();
    let radio: &'static Controller<'_> = RADIO.init(esp_radio::init().unwrap());
    let bluetooth = peripherals.BT;
    let connector = BleConnector::new(&radio, bluetooth, Default::default()).unwrap();
    let controller: ExternalController<_, 20> = ExternalController::new(connector);
    info!("Init'ing button task");
    spawner.spawn(button_task(input_btn, rtc, state)).unwrap();
    info!("Init'ing bluetooth task");
    spawner.spawn(run(controller, state)).unwrap();

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
