use defmt::info;
use embassy_time::Timer;
use esp_hal::{
    gpio::Input,
    rtc_cntl::{Rtc, sleep::GpioWakeupSource},
};

use crate::data::State;

pub struct ButtonDevices<'a> {
    pub button: &'a mut Input<'a>,
    pub rtc: &'a mut Rtc<'a>,
}

#[embassy_executor::task]
pub async fn button_task(
    button: &'static mut Input<'static>,
    rtc: &'static mut Rtc<'static>,
    _state: &'static State,
) {
    loop {
        info!("Waiting now");
        button.wait_for_falling_edge().await; // button pressed
        info!("Button pressed");

        // Short press: stop BLE

        // Long press (e.g., 3 sec)
        let mut held = true;
        Timer::after_secs(3).await;
        if button.is_high() {
            held = false; // released before long press
        }
        if held {
            info!("Long press detected: deep sleep!");
            rtc.sleep_deep(&[&GpioWakeupSource::new()]); // sleep for 1 hour
        }
    }
}
