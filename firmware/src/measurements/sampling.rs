use core::time::Duration;

use alloc::format;
use bytemuck::{AnyBitPattern, NoUninit};
use defmt::{Debug2Format, error, info};
use embassy_time::{Instant, Timer};
use esp_hal::{
    gpio, peripherals,
    rom::software_reset,
    rtc_cntl::{
        Rtc,
        sleep::{RtcioWakeupSource, TimerWakeupSource, WakeupLevel},
    },
};
use heapless::Vec;
use jiff::Timestamp;
use serde::{Deserialize, Serialize};

use crate::{
    PowerState, SAMPLE_BUFFER, SAMPLES_PER_BUFFER,
    data::Devices,
    measurements::{MeasurementResult, measure_once},
    storage::{MAX_NVS_VALUE, Nvs},
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
}

pub const MEAS_SIZE: usize = size_of::<Measurement>();

impl Measurement {
    fn from_reading(reading: MeasurementResult) -> Self {
        Self {
            temp_p: (reading.temperature.temperature * 100.0) as i32,
            pressure: (reading.pressure.pressure * 1000.0) as u32,
            temp_t: (reading.temperature.temperature * 100.0) as i32,
            humidity: reading.temperature.humidity as u16,
            co2: reading.co2.co2,
            voc: reading.voc.value,
        }
    }
}

pub async fn record_sample(devices: &'static Devices<'static>, beginning: Instant, nvs: &mut Nvs) {
    let mut rtc = Rtc::new(unsafe { peripherals::LPWR::steal() });

    // Ensure the CO2 doesn't get sampled more than every 5 secs to keep the algorithm working (specified by datasheet)
    let now = Timestamp::from_microsecond(rtc.current_time_us() as i64).unwrap();
    let parsed_ts = Timestamp::from_microsecond(unsafe { crate::LAST_CO2_SAMPLE } as i64).unwrap();
    let mut measure_co2 = false;
    if unsafe { crate::LAST_CO2_SAMPLE } == 0 {
        measure_co2 = true;
    } else {
        let elapsed_secs = now.duration_since(parsed_ts).as_secs();
        if elapsed_secs >= 5 {
            measure_co2 = true;
            unsafe { crate::LAST_CO2_SAMPLE = now.as_microsecond() as u64 };
        }
    }

    let reading = measure_once(devices, measure_co2).await;
    let measurement = Measurement::from_reading(reading);
    save(measurement, nvs).await;

    let mut pin = unsafe { peripherals::GPIO3::steal() };
    let wakeup_pins: &mut [(&mut dyn gpio::RtcPinWithResistors, WakeupLevel)] =
        &mut [(&mut pin, WakeupLevel::Low)];
    let wakeup_gpio = RtcioWakeupSource::new(wakeup_pins);
    let elapsed = embassy_time::Instant::now() - beginning;
    let rmng;
    let timer =
        if unsafe { crate::MEASUREMENT_SAMPLES_TAKEN >= crate::MEASUREMENT_SAMPLES_REQUESTED } {
            unsafe { crate::POWER_STATE = PowerState::BluetoothMode as i8 };
            unsafe { crate::NEEDS_SAMPLES_WRITTEN_TO_NVS = 1 };
            rmng = Duration::from_millis(20);
            TimerWakeupSource::new(rmng)
        } else {
            let interval = Duration::from_secs(unsafe { crate::SAMPLE_EVERY_SECONDS } as u64);
            let elapsed_dur = Duration::from_millis(elapsed.as_millis());
            rmng = interval
                .checked_sub(elapsed_dur)
                .unwrap_or(Duration::from_millis(10));
            TimerWakeupSource::new(rmng)
        };
    info!("Sleeping...");
    Timer::after_millis(rmng.as_millis() as u64).await;
    info!("Resetting...");
    software_reset();
    rtc.sleep_deep(&[&wakeup_gpio, &timer]);
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
    let _ = nvs.invalidate_key(bytes_nvs_key).await;
    info!("Len: {}", measurements.len());
    buffer[0..2].copy_from_slice(&(measurements.len() as u16).to_le_bytes());
    info!(
        "bm: {}",
        bytemuck::cast_slice::<Measurement, u8>(&measurements).len()
    );
    buffer[2..total_len].copy_from_slice(meas_bytes);

    nvs.append_key(
        bytes_nvs_key, // b"sample_0",
        &buffer[..total_len],
    )
    .await
    .unwrap();
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
