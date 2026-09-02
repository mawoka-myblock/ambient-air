use embassy_sync::pubsub::PubSubBehavior;
use embassy_time::Timer;

use crate::{
    BT_IS_CONNECTED_STATE, COMMAND_CHANNEL, Commands, MEASUREMENT_SIGNAL, POWER_STATE, PowerState,
    SleepOptions, data::Devices, leds::blink_n, measurements::MeasurementResult,
};

fn blink_level(pct: i8) -> u8 {
    match pct {
        0..=19 => 1,
        20..=39 => 2,
        40..=59 => 3,
        60..=79 => 4,
        _ => 5,
    }
}

pub async fn show_battery_percentage() {
    let d: MeasurementResult = loop {
        let r = MEASUREMENT_SIGNAL.anon_receiver().try_get();
        if let Some(d) = r {
            break d;
        } else {
            Timer::after_millis(100).await;
        }
    };
    let cmd_pub = COMMAND_CHANNEL.immediate_publisher();
    let charge_pct = d.battery.percentage;
    let lvl = blink_level(charge_pct);
    if lvl == 1 {
        blink_n(1, 5, 100, 100).await;
    } else {
        cmd_pub.publish_immediate(Commands::Led(crate::leds::LedCommand::Set {
            led: 1,
            level: 100,
        }));
    }
    blink_n(2, lvl, 300, 200).await;

    cmd_pub.publish_immediate(Commands::Led(crate::leds::LedCommand::SetAll(0)));
}

pub async fn deep_sleep_forever_on_low_battery(devices: &'static Devices<'static>) {
    let bat_pct = { devices.bq27441.lock().await.soc_percent().await.unwrap() };
    if bat_pct > 5 {
        return;
    }
    devices.sgp40.lock().await.turn_heater_off().await.unwrap();
    devices.stcc4.lock().await.enter_sleep_mode().await.unwrap();
    COMMAND_CHANNEL.publish_immediate(Commands::Sleep(SleepOptions {
        allow_buttons: false,
        wake_in_ms: Some(60 * 60 * 24 * 365), // one year
    }));
}

#[embassy_executor::task]
pub async fn auto_sleep() {
    Timer::after_secs(300).await;
    let mut s = BT_IS_CONNECTED_STATE.dyn_receiver().unwrap();
    loop {
        let is_connected = s.try_get().unwrap_or(false);
        if is_connected {
            Timer::after_secs(10).await;
            continue;
        }

        unsafe {
            POWER_STATE = PowerState::SensorActiveSleep as i8;
        }
        Timer::after_millis(290).await;
        COMMAND_CHANNEL.publish_immediate(Commands::Sleep(SleepOptions {
            wake_in_ms: Some(20),
            allow_buttons: true,
        }));
        Timer::after_secs(5).await;
    }
}
