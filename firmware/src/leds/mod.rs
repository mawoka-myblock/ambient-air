use defmt::Format;
use embassy_time::Timer as EmbassyTimer;
use esp_hal::{
    gpio::interconnect::PeripheralOutput,
    ledc::{
        Ledc, LowSpeed,
        channel::{self, Channel as LedcChannel, ChannelIFace as _},
        timer::Timer,
    },
};

use crate::{COMMAND_CHANNEL, Commands};

pub struct Leds<'a> {
    led_1: LedcChannel<'a, LowSpeed>,
    led_2: LedcChannel<'a, LowSpeed>,
}

impl<'a> Leds<'a> {
    pub fn new(
        ledc: &'static mut Ledc<'a>,
        tmr: &'a mut Timer<'a, LowSpeed>,
        led_1: impl PeripheralOutput<'static>,
        led_2: impl PeripheralOutput<'static>,
    ) -> Self {
        let mut channel0 = ledc.channel::<LowSpeed>(channel::Number::Channel0, led_1);
        channel0
            .configure(channel::config::Config {
                timer: tmr,
                duty_pct: 0,
                drive_mode: esp_hal::gpio::DriveMode::PushPull,
            })
            .unwrap();
        let mut channel1 = ledc.channel::<LowSpeed>(channel::Number::Channel2, led_2);
        channel1
            .configure(channel::config::Config {
                timer: tmr,
                duty_pct: 0,
                drive_mode: esp_hal::gpio::DriveMode::PushPull,
            })
            .unwrap();
        Self {
            led_1: channel0,
            led_2: channel1,
        }
    }

    pub async fn set_single(&self, target: u8, level: u8) {
        let t = match target {
            1 => &self.led_1,
            2 => &self.led_2,
            _ => panic!("Undefined target!"),
        };
        t.set_duty(level).unwrap();
    }

    pub async fn fade_single(&self, fc: FadeConfig, target: u8) {
        let t = match target {
            1 => &self.led_1,
            2 => &self.led_2,
            _ => panic!("Undefined target!"),
        };
        t.start_duty_fade(fc.start_pct, fc.end_pct, fc.fade_dur)
            .unwrap();
    }
}
#[derive(Debug, Format, Clone, Copy)]
pub struct FadeConfig {
    pub start_pct: u8,
    pub end_pct: u8,
    /// in ms
    pub fade_dur: u16,
}

#[derive(Debug, Format, Clone, Copy)]
pub enum LedCommand {
    SetAll(u8),
    Set { led: u8, level: u8 },
    Fade((FadeConfig, u8)),
}

#[embassy_executor::task]
pub async fn led_task(leds: &'static mut Leds<'static>) {
    let mut cmd_recv = COMMAND_CHANNEL.subscriber().unwrap();
    loop {
        if let Commands::Led(led_command) = cmd_recv.next_message_pure().await {
            match led_command {
                LedCommand::SetAll(d) => {
                    leds.set_single(1, d).await;
                    leds.set_single(2, d).await;
                }
                LedCommand::Fade(d) => leds.fade_single(d.0, d.1).await,
                LedCommand::Set { led, level } => leds.set_single(led, level).await,
            };
        }
    }
}

pub async fn blink_n(led: u8, n: u8, on_ms: u64, off_ms: u64) {
    let publisher = COMMAND_CHANNEL.immediate_publisher();

    for i in 0..n {
        publisher.publish_immediate(Commands::Led(LedCommand::Set { led, level: 100 }));
        EmbassyTimer::after_millis(on_ms).await;
        publisher.publish_immediate(Commands::Led(LedCommand::Set { led, level: 0 }));
        if i < n - 1 {
            EmbassyTimer::after_millis(off_ms).await;
        }
    }
}
/*
// New function to execute unique LED patterns for state transitions
pub async fn indicate_state_change(system_mode: SystemMode) {
    let ad = embassy_time::Duration::from_millis(200);
    let pa = embassy_time::Duration::from_millis(500);

    match system_mode {
        SystemMode::Initializing => {
            // Fast, single pulse
            COMMAND_CHANNEL
                .immediate_publisher()
                .publish_immediate(Commands::Led(LedCommand::Set { led: 1, level: 255 }));
            EmbassyTimer::after(ad).await;
            COMMAND_CHANNEL
                .immediate_publisher()
                .publish_immediate(Commands::Led(LedCommand::Set { led: 2, level: 255 }));
            EmbassyTimer::after(ad).await;
        }
        SystemMode::ActiveMeasurement => {
            // Fast, alternating flashes
            loop {
                COMMAND_CHANNEL
                    .immediate_publisher()
                    .publish_immediate(Commands::Led(LedCommand::Set { led: 1, level: 255 }));
                EmbassyTimer::after(ad).await;
                COMMAND_CHANNEL
                    .immediate_publisher()
                    .publish_immediate(Commands::Led(LedCommand::Set { led: 2, level: 255 }));
                EmbassyTimer::after(ad).await;
                break;
            }
        }
        SystemMode::BluetoothConnected => {
            // Slow, steady breathing effect (simulated by gentle fading)
            COMMAND_CHANNEL
                .immediate_publisher()
                .publish_immediate(Commands::Led(LedCommand::Fade((
                    FadeConfig {
                        start_pct: 0,
                        end_pct: 100,
                        fade_dur: 500,
                    },
                    1,
                ))));
            EmbassyTimer::after_millis(1500).await;
            COMMAND_CHANNEL
                .immediate_publisher()
                .publish_immediate(Commands::Led(LedCommand::Fade((
                    FadeConfig {
                        start_pct: 0,
                        end_pct: 100,
                        fade_dur: 500,
                    },
                    2,
                ))));
            EmbassyTimer::after_millis(1500).await;
        }
        SystemMode::MonitoringSleep => {
            // Slow, gentle pulse
            COMMAND_CHANNEL
                .immediate_publisher()
                .publish_immediate(Commands::Led(LedCommand::Set { led: 1, level: 50 }));
            EmbassyTimer::after_millis(1000).await;
            COMMAND_CHANNEL
                .immediate_publisher()
                .publish_immediate(Commands::Led(LedCommand::Set { led: 1, level: 0 }));
            EmbassyTimer::after_millis(1000).await;
        }
        SystemMode::DeepSleep => {
            // All off
        }
    }
}
 */
