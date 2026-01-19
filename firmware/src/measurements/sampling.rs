use core::time::Duration;

use alloc::format;
use bytemuck::{AnyBitPattern, NoUninit};
use defmt::{Debug2Format, error};
use embassy_time::Instant;
use esp_hal::{
    gpio, peripherals,
    rtc_cntl::{
        Rtc,
        sleep::{RtcioWakeupSource, TimerWakeupSource, WakeupLevel},
    },
};
use heapless::Vec;
use serde::{Deserialize, Serialize};

use crate::{
    PowerState, SAMPLE_BUFFER, SAMPLES_PER_BUFFER,
    data::Devices,
    measurements::{MeasurementResult, measure_once},
    storage::Nvs,
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
    embassy_time::Timer::after_millis(400).await;
    let reading = measure_once(devices).await;
    let measurement = Measurement::from_reading(reading);
    save(measurement, nvs).await;

    let mut rtc = Rtc::new(unsafe { peripherals::LPWR::steal() });
    let mut pin = unsafe { peripherals::GPIO3::steal() };
    let wakeup_pins: &mut [(&mut dyn gpio::RtcPinWithResistors, WakeupLevel)] =
        &mut [(&mut pin, WakeupLevel::Low)];
    let wakeup_gpio = RtcioWakeupSource::new(wakeup_pins);
    let elapsed = embassy_time::Instant::now() - beginning;
    let timer =
        if unsafe { crate::MEASUREMENT_SAMPLES_TAKEN == crate::MEASUREMENT_SAMPLES_REQUESTED } {
            unsafe { crate::POWER_STATE = PowerState::BluetoothMode as i8 }
            TimerWakeupSource::new(Duration::from_millis(20))
        } else {
            let interval = Duration::from_secs(unsafe { crate::SAMPLE_EVERY_SECONDS } as u64);
            let elapsed_dur = Duration::from_millis(elapsed.as_millis());
            let remaining = interval
                .checked_sub(elapsed_dur)
                .unwrap_or(Duration::from_millis(10));
            TimerWakeupSource::new(remaining)
        };
    rtc.sleep_deep(&[&wakeup_gpio, &timer]);
}

async fn save(mm: Measurement, nvs: &mut Nvs) {
    unsafe { crate::MEASUREMENT_SAMPLES_TAKEN += 1 }
    push_to_ram(mm, unsafe { crate::MEASUREMENT_SAMPLES_TAKEN } as usize);

    let need_to_aggregate_samples =
        // Check if we got SAMPLES_PER_BUFFER amount of samples in RAM or if there are no samples left, then we move into nvs
        unsafe { crate::MEASUREMENT_SAMPLES_TAKEN }
            % crate::SAMPLES_PER_BUFFER as i16
            == 0
            || unsafe { crate::MEASUREMENT_SAMPLES_TAKEN == crate::MEASUREMENT_SAMPLES_REQUESTED };
    if need_to_aggregate_samples {
        move_to_nvs(nvs, unsafe { crate::MEASUREMENT_SAMPLES_TAKEN } as usize).await;
    }
}

fn push_to_ram(m: Measurement, count: usize) {
    unsafe {
        let offset = (count - 1) * MEAS_SIZE;
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

async fn move_to_nvs(nvs: &mut Nvs, _sample: usize) {
    let mut measurements: Vec<Measurement, { crate::SAMPLES_PER_BUFFER }> = Vec::new();
    let count_to_copy =
        unsafe { crate::MEASUREMENT_SAMPLES_TAKEN.min(crate::SAMPLES_PER_BUFFER as i16) } as usize;
    for i in 0..count_to_copy {
        measurements.push(read_measurement(i)).unwrap();
    }
    let batch_index =
        unsafe { (crate::MEASUREMENT_SAMPLES_TAKEN - 1) / crate::SAMPLES_PER_BUFFER as i16 };
    let mut buffer: [u8; 1000] = [0; 1000];
    let _ = nvs.invalidate_key(b"sample_1").await;
    buffer[0..2].copy_from_slice(&(measurements.len() as u16).to_le_bytes());
    buffer[2..2 + measurements.len() * MEAS_SIZE]
        .copy_from_slice(bytemuck::cast_slice(&measurements));
    let nvs_key = format!("sample_{}", batch_index);
    let bytes_nvs_key = nvs_key.as_bytes();
    nvs.append_key(
        bytes_nvs_key, // b"sample_0",
        &buffer,
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
        Ok(d) => d,
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
