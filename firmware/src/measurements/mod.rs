pub mod lp;
pub mod sampling;
use embassy_time::{Duration, Instant, Timer};

use crate::data::{Co2Data, Devices, PressureData, State, TemperatureData, VocData};

#[embassy_executor::task]
pub async fn measure(state: &'static State, mut devices: Devices<'static>) {
    loop {
        let refresh_secs = {
            let s = state.config.lock().await;
            s.update_interval
        };
        let beginning = Instant::now();
        let d = measure_once(&mut devices).await;
        {
            let mut s = state.temperature.lock().await;
            s.humidity = d.temperature.humidity;
            s.temperature = d.temperature.temperature;
        }
        {
            let mut s = state.pressure.lock().await;
            s.pressure = d.pressure.pressure;
            s.temperature = d.pressure.temperature;
        }
        {
            let mut s = state.co2.lock().await;
            s.co2 = d.co2.co2;
        }
        {
            let mut s = state.voc.lock().await;
            s.value = d.voc.value;
            if s.readings_until_warmup_complete > 0 {
                s.readings_until_warmup_complete -= 1
            }
        }

        let time_passed = Instant::now() - beginning;
        let refresh_frequency_wanted = Duration::from_millis((refresh_secs * 1000) as u64);
        let sleep_time = if refresh_frequency_wanted > time_passed {
            refresh_frequency_wanted - time_passed
        } else {
            Duration::from_millis(0)
        };

        Timer::after(sleep_time).await;
        // info!(
        //     "Refresh took {:?}, now sleeping for {:?}",
        //     time_passed.as_millis(),
        //     sleep_time.as_millis()
        // );
    }
}

pub struct MeasurementResult {
    pub pressure: PressureData,
    pub temperature: TemperatureData,
    pub voc: VocData,
    pub co2: Co2Data,
}

pub async fn measure_once(devices: &mut Devices<'static>) -> MeasurementResult {
    let pressure = {
        let reading = devices.icp.read_pressure_and_temperature().await.unwrap();
        PressureData {
            pressure: reading.0,
            temperature: reading.1,
        }
    };
    let temperature = {
        let reading = devices.aht20.measure().await.unwrap();
        TemperatureData {
            humidity: reading.humidity,
            temperature: reading.temperature,
        }
    };
    let voc = {
        let reading = devices
            .sgp40
            .measure_voc_index_with_rht(
                (temperature.humidity * 1000.0) as u16,
                (temperature.temperature * 1000.0) as i16,
            )
            .await
            .unwrap() as i32;
        VocData {
            readings_until_warmup_complete: 0,
            value: reading,
        }
    };
    let co2 = {
        devices.stcc4.single_shot().await.unwrap();
        let (co2, _, _) = devices.stcc4.read_measurement().await.unwrap();
        Co2Data { co2: co2 }
    };
    MeasurementResult {
        co2,
        pressure,
        temperature,
        voc,
    }
}
