use core::time::Duration;

use alloc::format;
use bytemuck::{AnyBitPattern, NoUninit};
use defmt::{Debug2Format, error, info};
use embassy_time::Instant;
use esp_hal::{peripherals, rtc_cntl::Rtc};
use heapless::Vec;
use serde::{Deserialize, Serialize};

use crate::{
    PowerState, SAMPLE_BUFFER, SAMPLES_PER_BUFFER, SleepOptions,
    data::Devices,
    measurements::{MeasurementResult, measure_once},
    storage::{MAX_NVS_VALUE, Nvs},
    tasks::sleep::deep_sleep_basic_with_cfg,
};

#[repr(C, packed)]
#[derive(Copy, Clone, Debug, NoUninit, AnyBitPattern, Deserialize, Serialize)]
pub struct Measurement {
    temp_p: i32,   // 2623 -> 26.23°C
    pressure: u32, // 12345 -> 12.345 Pa
    temp_t: i32,   // 2623 -> 26.23°C
    humidity: u16, // 42 -> 42%
    co2: i16,
    voc: i32,
    ms_offset: u32, // ms since first measurement
}

pub const MEAS_SIZE: usize = size_of::<Measurement>();
const _: () = assert!(MEAS_SIZE == 24, "MEAS_SIZE is wrong!");

impl Measurement {
    fn from_reading(reading: MeasurementResult, ms_offset: u32) -> Self {
        Self {
            temp_p: (reading.temperature.temperature * 100.0) as i32,
            pressure: (reading.pressure.pressure * 1000.0) as u32,
            temp_t: (reading.temperature.temperature * 100.0) as i32,
            humidity: reading.temperature.humidity as u16,
            co2: reading.co2.co2,
            voc: reading.voc.value,
            ms_offset,
        }
    }
}
/// Record sample in sampling mode
/// Saves samples first to rtc memory, then to nvs
/// Goes to sleep later and wakes up when:
/// - timer expires (new sample)
/// - a button is pressed (stops measurement cycle)
pub async fn record_sample(devices: &'static Devices<'static>, beginning: Instant) -> ! {
    let mut rtc = Rtc::new(unsafe { peripherals::LPWR::steal() });

    // Ensure the CO2 doesn't get sampled more than every 5 secs to keep the algorithm working (specified by datasheet)
    let dur_since_pwrup = rtc.time_since_power_up();
    let now = dur_since_pwrup.as_millis();
    info!("Time since boot: {}", now);
    let last_raw = unsafe { crate::LAST_CO2_SAMPLE };
    let measure_co2 = if last_raw != 0 {
        now.saturating_sub(last_raw) >= 5000
    } else {
        true // uninitialized
    };
    if measure_co2 {
        unsafe {
            crate::LAST_CO2_SAMPLE = now;
        }
    }
    if (unsafe { crate::MEASUREMENT_SAMPLES_TAKEN } == 0) {
        unsafe { crate::FIRST_MEASUREMENT_TS = now }
    }
    let time_offset = (now - unsafe { crate::FIRST_MEASUREMENT_TS }) as u32;

    let reading = measure_once(devices, measure_co2, &dur_since_pwrup).await;
    let measurement = Measurement::from_reading(reading, time_offset);
    save(measurement, &mut *devices.nvs.lock().await).await;
    let elapsed = embassy_time::Instant::now() - beginning;

    let wakeup_in_ms: u64 =
        if unsafe { crate::MEASUREMENT_SAMPLES_TAKEN >= crate::MEASUREMENT_SAMPLES_REQUESTED } {
            unsafe { crate::POWER_STATE = PowerState::BluetoothMode as i8 };
            unsafe { crate::NEEDS_SAMPLES_WRITTEN_TO_NVS = 1 };
            20
        } else {
            let interval = Duration::from_secs(unsafe { crate::SAMPLE_EVERY_SECONDS } as u64);
            let elapsed_dur = Duration::from_millis(elapsed.as_millis());
            (interval
                .checked_sub(elapsed_dur)
                .unwrap_or(Duration::from_millis(10)))
            .as_millis() as u64
        };
    deep_sleep_basic_with_cfg(
        &mut rtc,
        &SleepOptions {
            allow_buttons: true,
            wake_in_ms: Some(wakeup_in_ms),
        },
    );
}

async fn save(mm: Measurement, nvs: &mut Nvs) {
    unsafe { crate::MEASUREMENT_SAMPLES_TAKEN += 1 }
    push_to_ram(mm, unsafe { crate::MEASUREMENT_SAMPLES_TAKEN } as usize);

    let need_to_aggregate_samples =
        // Check if we got SAMPLES_PER_BUFFER amount of samples in RAM or if there is no samples left, then we move into nvs
        unsafe { crate::MEASUREMENT_SAMPLES_TAKEN }
            % crate::SAMPLES_PER_BUFFER as i16
            == 0;
    if need_to_aggregate_samples {
        move_to_nvs(nvs).await;
    }
}

fn push_to_ram(m: Measurement, count: usize) {
    unsafe {
        info!("Count: {}", count);
        let index = (count - 1) % crate::SAMPLES_PER_BUFFER;
        let offset = index * MEAS_SIZE;
        let bytes: &[u8] = bytemuck::bytes_of(&m);
        SAMPLE_BUFFER[offset..offset + MEAS_SIZE].copy_from_slice(bytes);
    }
}

fn read_measurement(idx: usize) -> Measurement {
    unsafe {
        let offset = idx * MEAS_SIZE;
        *bytemuck::from_bytes::<Measurement>(&SAMPLE_BUFFER[offset..offset + MEAS_SIZE])
    }
}

pub async fn move_to_nvs(nvs: &mut Nvs) {
    let mut measurements: Vec<Measurement, { crate::SAMPLES_PER_BUFFER }> = Vec::new();
    let count_to_copy =
        unsafe { crate::MEASUREMENT_SAMPLES_TAKEN.min(crate::SAMPLES_PER_BUFFER as i16) } as usize;
    for i in 0..count_to_copy {
        measurements.push(read_measurement(i)).unwrap();
    }
    let batch_index =
        unsafe { (crate::MEASUREMENT_SAMPLES_TAKEN - 1) / crate::SAMPLES_PER_BUFFER as i16 };

    let meas_bytes = bytemuck::cast_slice::<Measurement, u8>(&measurements);
    let total_len = 2 + meas_bytes.len();
    let mut buffer: [u8; MAX_NVS_VALUE] = [0; MAX_NVS_VALUE];
    let nvs_key = format!("sample_{}", batch_index);
    let bytes_nvs_key = nvs_key.as_bytes();
    match nvs.invalidate_key(bytes_nvs_key).await {
        Ok(_) => (),
        Err(e) => defmt::error!("{:?}", Debug2Format(&e)),
    };
    info!("Len: {}", measurements.len());
    buffer[0..2].copy_from_slice(&(measurements.len() as u16).to_le_bytes());
    info!(
        "bm: {}",
        bytemuck::cast_slice::<Measurement, u8>(&measurements).len()
    );
    buffer[2..total_len].copy_from_slice(meas_bytes);

    match nvs
        .append_key(
            bytes_nvs_key, // b"sample_0",
            &buffer[..total_len],
        )
        .await
    {
        Ok(_) => (),
        Err(e) => defmt::error!("{:?}", Debug2Format(&e)),
    };
    let _ = match nvs.get_key(bytes_nvs_key).await {
        Ok(d) => d,
        Err(e) => {
            error!("{:?}", Debug2Format(&e));
            panic!("e")
        }
    };
    unsafe { SAMPLE_BUFFER = [0u8; SAMPLES_PER_BUFFER * MEAS_SIZE] }
}

pub async fn from_nvs(nvs: &mut Nvs, id: usize) -> Vec<Measurement, { crate::SAMPLES_PER_BUFFER }> {
    let nvs_key = format!("sample_{}", id);
    let bytes_nvs_key = nvs_key.as_bytes();
    let d = match nvs.get_key(bytes_nvs_key).await {
        Ok(d) => d.0,
        Err(e) => {
            error!("{:?}", Debug2Format(&e));
            panic!("Error!")
        }
    };
    let count = u16::from_le_bytes(d[0..2].try_into().unwrap()) as usize;

    // 3. Convert bytes into Measurements
    let mut measurements: Vec<Measurement, { SAMPLES_PER_BUFFER }> = Vec::new();
    for i in 0..count {
        let start = 2 + i * MEAS_SIZE;
        let end = start + MEAS_SIZE;
        let m: Measurement = *bytemuck::from_bytes(&d[start..end]);
        measurements.push(m).unwrap();
    }
    measurements
}
