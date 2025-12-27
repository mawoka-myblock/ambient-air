pub mod lp;
pub mod sampling;
pub mod sensors;
use embassy_time::{Duration, Instant, Timer};

use crate::data::{Battery, Co2Data, Devices, PressureData, State, TemperatureData, VocData};

#[embassy_executor::task]
pub async fn measure(state: &'static State, devices: &'static Devices<'static>) {
    loop {
        let refresh_secs = {
            let s = state.config.lock().await;
            s.update_interval
        };
        let beginning = Instant::now();
        let d = measure_once(devices).await;
        // info!("Measurement data: {:?}", Debug2Format(&d));
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
        {
            let mut s = state.battery.lock().await;
            s.percentage = d.battery.percentage;
            s.power = d.battery.power;
            s.voltage = d.battery.voltage;
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
#[derive(Debug)]
pub struct MeasurementResult {
    pub pressure: PressureData,
    pub temperature: TemperatureData,
    pub voc: VocData,
    pub co2: Co2Data,
    pub battery: Battery,
}

pub async fn measure_once(devices: &'static Devices<'static>) -> MeasurementResult {
    let pressure = {
        let reading: (f32, f32) = devices
            .icp
            .lock()
            .await
            .read_pressure_and_temperature()
            .await
            .unwrap();
        PressureData {
            pressure: reading.0,
            temperature: reading.1,
        }
    };
    let temperature = {
        let reading = devices.aht20.lock().await.measure().await.unwrap();
        TemperatureData {
            humidity: reading.humidity,
            temperature: reading.temperature,
        }
    };
    let voc = {
        if unsafe { crate::SGP40_ENABLED == 1 } {
            let reading = devices
                .sgp40
                .lock()
                .await
                .measure_voc_index_with_rht(
                    (temperature.humidity * 1000.0) as u16,
                    (temperature.temperature * 1000.0) as i16,
                )
                .await
                .unwrap_or(0) as i32;
            VocData {
                readings_until_warmup_complete: 0,
                value: reading,
            }
        } else {
            VocData {
                readings_until_warmup_complete: 0,
                value: 0,
            }
        }
    };
    let co2 = {
        let mut stcc4 = devices.stcc4.lock().await;
        stcc4
            .set_rht_compensation(temperature.temperature, temperature.humidity)
            .await
            .unwrap();
        stcc4
            .set_pressure_compensation((pressure.pressure * 1000.0) as i32)
            .await
            .unwrap();
        stcc4.single_shot(true).await.unwrap();
        let (co2, _, _) = stcc4.read_measurement().await.unwrap();
        Co2Data { co2 }
    };
    let battery = {
        let mut bq = devices.bq27441.lock().await;
        let avg_power = bq.average_power_mw().await.unwrap();
        // let avg_current = bq.avg_current_ma().await.unwrap();
        let voltage = bq.voltage_mv().await.unwrap();
        let soc = bq.soc_percent().await.unwrap();
        Battery {
            percentage: soc as i8,
            power: avg_power,
            voltage,
        }
    };
    MeasurementResult {
        co2,
        pressure,
        temperature,
        voc,
        battery,
    }
}
