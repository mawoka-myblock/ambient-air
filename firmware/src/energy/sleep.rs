use core::time::Duration;

use defmt::error;
use esp_hal::{
    gpio, peripherals,
    rtc_cntl::{
        Rtc,
        sleep::{RtcioWakeupSource, TimerWakeupSource, WakeupLevel},
    },
};

use crate::{
    PowerState, SGP40_ENABLED, STCC4_SAMPLE_RATE,
    data::Devices,
    measurements::sensors::deep_sleep,
    nvs_keys::{SGP40_ENABLED_KEY, STCC4_SAMPLE_RATE_KEY},
    storage::Nvs,
};

#[derive(Debug, PartialEq, Eq)]
pub enum SleepState {
    /// lowest power consumption
    BatteryLow,
    /// Sensors don't sample, no wakeup time defined
    DeepSleep,
    /// ESP in DeepSleep, CO2 and VOx sensors still sample
    Standby,
    /// Don't sleep, just reset by waking from Deep Sleep
    Reset,
    /// Active Sampling
    Sample,
}

/// This fn gets the FLASH, LPWR (Rtc) and the GPIO3 peripheral out of thin air.
/// Make sure it's freed before calling this fn
///
/// The `devices` are needed when BatteryLow is run
pub async fn go_sleep(state: SleepState, devices: Option<&mut Devices<'_>>) {
    let nvs = Nvs::new(crate::NVS_OFFSET, crate::NVS_SIZE, unsafe {
        peripherals::FLASH::steal()
    })
    .unwrap();
    let stcc4_sampling_rate: i16 = match nvs.get_key(STCC4_SAMPLE_RATE_KEY).await {
        Ok(bytes) => i16::from_le_bytes([bytes[0], bytes[1]]),
        Err(_) => 600,
    };
    let sgp40_enabled = nvs
        .get_key(SGP40_ENABLED_KEY)
        .await
        .ok()
        .and_then(|d| d.first().copied())
        .map(|v| v != 0)
        .unwrap_or(false);
    unsafe {
        STCC4_SAMPLE_RATE = stcc4_sampling_rate;
        SGP40_ENABLED = match sgp40_enabled {
            true => 1,
            false => 0,
        }
    };
    let mut rtc = Rtc::new(unsafe { peripherals::LPWR::steal() });
    let mut pin = unsafe { peripherals::GPIO3::steal() };
    let wakeup_pins: &mut [(&mut dyn gpio::RtcPinWithResistors, WakeupLevel)] =
        &mut [(&mut pin, WakeupLevel::Low)];
    let wakeup_gpio = RtcioWakeupSource::new(wakeup_pins);

    if state == SleepState::Reset {
        rtc.sleep_deep(&[&TimerWakeupSource::new(Duration::from_millis(20))]);
    }

    if state == SleepState::BatteryLow {
        deep_sleep(devices.unwrap()).await;
        rtc.sleep_deep(&[]);
    }

    if state == SleepState::DeepSleep {
        deep_sleep(devices.unwrap()).await;
        rtc.sleep_deep(&[&wakeup_gpio]);
    }

    if state == SleepState::Standby {
        unsafe { crate::POWER_STATE = PowerState::SensorActiveSleep as i8 }
        let timer = match sgp40_enabled {
            true => TimerWakeupSource::new(Duration::from_secs(1)),
            false => TimerWakeupSource::new(Duration::from_secs(stcc4_sampling_rate as u64)),
        };
        rtc.sleep_deep(&[&wakeup_gpio, &timer]);
    }

    if state == SleepState::Sample {
        unsafe { crate::POWER_STATE = PowerState::SampleMode as i8 }
        rtc.sleep_deep(&[&TimerWakeupSource::new(Duration::from_millis(20))]);
    }
}

pub async fn go_sleep_without_devices(state: SleepState) {
    let mut rtc = Rtc::new(unsafe { peripherals::LPWR::steal() });
    let mut pin = unsafe { peripherals::GPIO3::steal() };
    let wakeup_pins: &mut [(&mut dyn gpio::RtcPinWithResistors, WakeupLevel)] =
        &mut [(&mut pin, WakeupLevel::Low)];
    let wakeup_gpio = RtcioWakeupSource::new(wakeup_pins);
    if state == SleepState::Reset {
        rtc.sleep_deep(&[&TimerWakeupSource::new(Duration::from_millis(20))]);
    }

    if state == SleepState::BatteryLow {
        error!("Not setting peripherals!");
        rtc.sleep_deep(&[]);
    }

    if state == SleepState::DeepSleep {
        error!("Not setting peripherals!");
        rtc.sleep_deep(&[&wakeup_gpio]);
    }

    if state == SleepState::Standby {
        unsafe { crate::POWER_STATE = PowerState::SensorActiveSleep as i8 }
        let timer = match unsafe { SGP40_ENABLED } {
            1 => TimerWakeupSource::new(Duration::from_secs(1)),
            2 => TimerWakeupSource::new(Duration::from_secs(unsafe { STCC4_SAMPLE_RATE } as u64)),
            _ => TimerWakeupSource::new(Duration::from_secs(1)),
        };
        rtc.sleep_deep(&[&wakeup_gpio, &timer]);
    }

    if state == SleepState::Sample {
        unsafe { crate::POWER_STATE = PowerState::SampleMode as i8 }
        rtc.sleep_deep(&[&TimerWakeupSource::new(Duration::from_millis(20))]);
    }
}
