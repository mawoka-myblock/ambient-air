use defmt::{Debug2Format, error};

use crate::data::Devices;

pub mod sleep;

/// Either enables or disables SGP40 based on NVS data
pub async fn set_sgp40(devices: &Devices<'static>) {
    let sgp40_enabled = devices
        .nvs
        .lock()
        .await
        .get_key(crate::nvs_keys::SGP40_ENABLED_KEY)
        .await
        .ok()
        .and_then(|d| d.0.first().copied())
        .map(|v| v != 0)
        .unwrap_or(false);
    unsafe {
        crate::SGP40_ENABLED = match sgp40_enabled {
            true => 1,
            false => 0,
        }
    }
    if !sgp40_enabled {
        match devices.sgp40.lock().await.turn_heater_off().await {
            Ok(_) => (),
            Err(e) => error!("{:?}", Debug2Format(&e)),
        };
    }
}
