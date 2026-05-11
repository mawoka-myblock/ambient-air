use core::time::Duration;

use async_button::{Button, ButtonConfig, ButtonEvent};
use defmt::info;
use embassy_time::Timer;
use esp_hal::{
    gpio::{self, Input, InputConfig},
    peripherals,
    rtc_cntl::{
        Rtc,
        sleep::{RtcioWakeupSource, TimerWakeupSource, WakeupLevel},
    },
};

use crate::{POWER_STATE, PowerState};

#[embassy_executor::task]
pub async fn button_task() {
    let input_btn = Input::new(
        unsafe { peripherals::GPIO3::steal() },
        InputConfig::default().with_pull(esp_hal::gpio::Pull::Up),
    );
    Input::new(
        unsafe { peripherals::GPIO4::steal() },
        InputConfig::default().with_pull(esp_hal::gpio::Pull::Up),
    );

    let mut btn = Button::new(input_btn, ButtonConfig::default());
    loop {
        match btn.update().await {
            ButtonEvent::LongPress => {
                info!("Long press detected: deep sleep!");
                break;
            }
            ButtonEvent::ShortPress { count } => {
                if count == 2 {
                    unsafe {
                        POWER_STATE = PowerState::SensorActiveSleep as i8;
                    }
                    let mut rtc = Rtc::new(unsafe { peripherals::LPWR::steal() });
                    rtc.sleep_deep(&[&TimerWakeupSource::new(Duration::from_millis(20))]);
                }
            }
        }
    }
    Timer::after_millis(1000).await;
    let mut rtc = Rtc::new(unsafe { peripherals::LPWR::steal() });
    let mut pin_1 = unsafe { peripherals::GPIO3::steal() };
    let mut pin_2 = unsafe { peripherals::GPIO4::steal() };
    let wakeup_pins: &mut [(&mut dyn gpio::RtcPinWithResistors, WakeupLevel)] = &mut [
        (&mut pin_1, WakeupLevel::Low),
        (&mut pin_2, WakeupLevel::Low),
    ];
    let wakeup_gpio = RtcioWakeupSource::new(wakeup_pins);
    let wakeup_timer = TimerWakeupSource::new(Duration::from_mins(1));
    Timer::after_millis(100).await;
    rtc.sleep_deep(&[&wakeup_gpio, &wakeup_timer]);
}

// 7
