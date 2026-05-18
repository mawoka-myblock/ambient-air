use core::time::Duration;

use defmt::info;
use embassy_futures::join::{join, join5};
use esp_hal::peripherals::LPWR;
use esp_hal::rtc_cntl::Rtc;

use crate::SleepOptions;
use crate::data::Devices;
use crate::measurements::voc::{restore_voc_state, store_voc_state};
use crate::tasks::sleep::deep_sleep_basic_with_cfg;
use crate::tasks::stcc4::{Stcc4State, get_stcc4_state};

/// This fn polls:
/// - the AHT20 for temperature
/// - the ICP20100 for pressure
/// - the STCC4 for Co2 with the given data to keep the algorithm healthy
/// - if enabled, the SGP40 for VOC
///
/// Goes to sleep afterwards, wakes up from:
/// - timer for next lp measurement cycle
/// - both buttons
pub async fn lp_measurement(
    devices: &'static Devices<'static>,
    rtc_peripheral: LPWR<'static>,
) -> ! {
    let mut devs = join5(
        devices.aht20.lock(),
        devices.icp.lock(),
        devices.stcc4.lock(),
        devices.sgp40.lock(),
        devices.bq27441.lock(),
    )
    .await;
    let mut stcc4 = devs.2;

    let mut rtc = Rtc::new(rtc_peripheral);

    let (reading, pressure) = {
        let (read_res, pres_res) = join(devs.0.measure(), devs.1.read_pressure()).await;
        (read_res.unwrap(), pres_res.unwrap())
    };

    let _ = stcc4
        .set_pressure_compensation((pressure * 1000.0) as i32)
        .await;
    let _ = stcc4
        .set_rht_compensation(reading.temperature, reading.humidity)
        .await;
    let stcc4_state = get_stcc4_state(&rtc.time_since_power_up());
    match stcc4_state {
        Stcc4State::InContinous => (),
        Stcc4State::NeedsContinousStop => {
            let _ = stcc4.stop_continuous().await;
        }
        Stcc4State::Normal => {
            let _ = stcc4.single_shot(false).await;
        }
    };
    let mut sleep_dur = Duration::from_secs((unsafe { crate::STCC4_SAMPLE_RATE } as u64).max(5));
    if unsafe { crate::SGP40_ENABLED == 1 } {
        info!("SGP enabled");
        let mut sgp40 = devs.3;
        let restored_voc_state = restore_voc_state();
        if restored_voc_state.uptime > 0.0 {
            sgp40.set_algorithm_state(&restored_voc_state);
        }
        sgp40
            .measure_voc_index_with_rht(
                (reading.humidity * 1000.0) as u16,
                (reading.temperature * 1000.0) as i16,
            )
            .await
            .unwrap();
        store_voc_state(&sgp40.dump_algorithm_state());
        unsafe { crate::SGP40_READINGS += 1 }
        sleep_dur = Duration::from_secs(1);
    }
    deep_sleep_basic_with_cfg(
        &mut rtc,
        &SleepOptions {
            allow_buttons: true,
            wake_in_ms: Some(sleep_dur.as_millis() as u64),
        },
    );
    // todo!("Sleep!");
}
