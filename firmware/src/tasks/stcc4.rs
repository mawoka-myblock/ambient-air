use defmt::{Debug2Format, Format, debug, info, unwrap};
use embassy_time::Timer;
use esp_hal::{peripherals, rtc_cntl::Rtc, time::Duration};

use crate::{COMMAND_CHANNEL, Commands, bluetooth::services::Co2Command, data::Devices};

#[embassy_executor::task]
pub async fn stcc4_task(devices: &'static Devices<'static>) {
    let mut cmd_listener = unwrap!(COMMAND_CHANNEL.subscriber());
    loop {
        let m = cmd_listener.next_message_pure().await;
        let Commands::Stcc4(c) = m else {
            continue;
        };
        let current_ts = Rtc::new(unsafe { peripherals::LPWR::steal() })
            .time_since_power_up()
            .as_secs();
        let mut stcc4 = devices.stcc4.lock().await;
        match c {
            Co2Command::FactoryReset => {
                info!("Factory-resetting stcc4");
                unsafe { crate::STTCC4_CONT_UNTIL_S = current_ts as u32 + 60 * 60 * 12 }; // continous mode needed for 12h
                match stcc4.factory_reset().await {
                    Ok(_) => (),
                    Err(e) => defmt::error!("{:?}", Debug2Format(&e)),
                };
            }
            Co2Command::PerformConditioning => {
                info!("Preconditioning stcc4");
                unsafe { crate::STTCC4_CONT_UNTIL_S = current_ts as u32 + 60 * 60 }; // continous mode needed for 1h

                match stcc4.conditioning(true).await {
                    Ok(_) => (),
                    Err(e) => defmt::error!("{:?}", Debug2Format(&e)),
                };
                Timer::after_millis(300).await;
                info!("Precon done");
            }
        }
        info!("crate::STTCC4_CONT_UNTIL_S: {:?}", unsafe {
            crate::STTCC4_CONT_UNTIL_S
        });
        match stcc4.start_continuous().await {
            Ok(_) => (),
            Err(e) => defmt::error!("{:?}", Debug2Format(&e)),
        };
    }
}

#[derive(Debug, Format, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Stcc4State {
    InContinous,
    Normal,
    NeedsContinousStop,
}

/// Call to check if Stcc4 may be in intial setup or conditioning mode and this is in continous sampling mode
/// The STCC4_CONT_UNTIL_S is reset here.
pub fn get_stcc4_state(now_ts: &Duration) -> Stcc4State {
    let ts = unsafe { crate::STTCC4_CONT_UNTIL_S };
    let now_s = now_ts.as_secs() as u32;
    debug!("TS: {}, Now: {}", ts, now_s);

    if ts == 0 {
        Stcc4State::Normal
    } else if now_s < ts {
        Stcc4State::InContinous
    } else if now_s < ts.saturating_add(60 * 11) {
        unsafe { crate::STTCC4_CONT_UNTIL_S = 0 }
        Stcc4State::NeedsContinousStop
    } else {
        Stcc4State::Normal
    }
}
