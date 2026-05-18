use async_button::{Button, ButtonConfig, ButtonEvent};
use defmt::info;
use embassy_time::Timer;
use esp_hal::{
    gpio::{Input, InputConfig},
    peripherals::{self},
};

use crate::{
    COMMAND_CHANNEL, Commands, POWER_STATE, PowerState, SleepOptions,
    leds::{FadeConfig, LedCommand},
};

#[embassy_executor::task]
pub async fn button_task(p1: peripherals::GPIO3<'static>, p2: peripherals::GPIO4<'static>) {
    let input_btn = Input::new(
        p1,
        InputConfig::default().with_pull(esp_hal::gpio::Pull::Up),
    );
    Input::new(
        p2,
        InputConfig::default().with_pull(esp_hal::gpio::Pull::Up),
    );

    let cmd_pub = COMMAND_CHANNEL.immediate_publisher();

    let mut btn = Button::new(input_btn, ButtonConfig::default());
    loop {
        match btn.update().await {
            ButtonEvent::LongPress => {
                info!("Long press detected: deep sleep!");
                break;
            }
            ButtonEvent::ShortPress { count } => {
                if count == 2 {
                    cmd_pub.publish_immediate(Commands::Led(LedCommand::Fade((
                        FadeConfig {
                            start_pct: 0,
                            end_pct: 100,
                            fade_dur: 50,
                        },
                        1,
                    ))));
                    Timer::after_millis(50).await;
                    cmd_pub.publish_immediate(Commands::Led(LedCommand::Fade((
                        FadeConfig {
                            start_pct: 100,
                            end_pct: 0,
                            fade_dur: 300,
                        },
                        1,
                    ))));
                    unsafe {
                        POWER_STATE = PowerState::SensorActiveSleep as i8;
                    }
                    Timer::after_millis(290).await;
                    cmd_pub.publish_immediate(Commands::Sleep(SleepOptions {
                        wake_in_ms: Some(20),
                        allow_buttons: true,
                    }));
                }
            }
        }
    }
    Timer::after_millis(1000).await;
    COMMAND_CHANNEL
        .immediate_publisher()
        .publish_immediate(Commands::Sleep(SleepOptions {
            wake_in_ms: Some(60 * 1000),
            allow_buttons: true,
        }));
}

// 7
