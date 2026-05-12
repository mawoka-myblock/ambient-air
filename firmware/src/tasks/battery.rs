use embassy_time::Timer;

use crate::{
    COMMAND_CHANNEL, Commands, MEASUREMENT_SIGNAL, leds::blink_n, measurements::MeasurementResult,
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
