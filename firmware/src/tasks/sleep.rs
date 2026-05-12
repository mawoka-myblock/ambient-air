use core::time::Duration;

use crate::{COMMAND_CHANNEL, Commands, SleepOptions, data::Devices};
use defmt::unwrap;
use esp_hal::{
    gpio,
    peripherals::{self, LPWR},
    rtc_cntl::{
        Rtc,
        sleep::{RtcioWakeupSource, TimerWakeupSource, WakeSource, WakeupLevel},
    },
};

#[embassy_executor::task]
pub async fn sleep_task(devices: &'static Devices<'static>, rtc_peri: LPWR<'static>) {
    let mut cmd_listener = unwrap!(COMMAND_CHANNEL.subscriber());
    let mut rtc = Rtc::new(rtc_peri);
    loop {
        if let Commands::Sleep(d) = cmd_listener.next_message_pure().await {
            deep_sleep_basic_with_cfg(&mut rtc, &d);
        }
    }
}

pub fn deep_sleep_basic_with_cfg<'a>(rtc: &'a mut Rtc<'a>, d: &SleepOptions) -> ! {
    let mut pin_1;
    let mut pin_2;
    let mut wakeup_pins_storage: [(&mut dyn gpio::RtcPinWithResistors, WakeupLevel); 2];
    let timer_ws;
    let gpio_ws;
    let mut wake_sources: heapless::Vec<&dyn WakeSource, 2> = heapless::Vec::new();

    if d.allow_buttons {
        pin_1 = unsafe { peripherals::GPIO3::steal() };
        pin_2 = unsafe { peripherals::GPIO4::steal() };
        wakeup_pins_storage = [
            (&mut pin_1, WakeupLevel::Low),
            (&mut pin_2, WakeupLevel::Low),
        ];
        gpio_ws = Some(RtcioWakeupSource::new(&mut wakeup_pins_storage));
        let _ = wake_sources.push(gpio_ws.as_ref().unwrap());
    }
    if let Some(sleep_dur_ms) = d.wake_in_ms {
        timer_ws = Some(TimerWakeupSource::new(Duration::from_millis(sleep_dur_ms)));
        let _ = wake_sources.push(timer_ws.as_ref().unwrap());
    }
    rtc.sleep_deep(wake_sources.as_slice())
}
