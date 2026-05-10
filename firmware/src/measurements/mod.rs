pub mod lp;
pub mod sampling;
pub mod sensors;
pub mod voc;
use defmt::{Debug2Format, info};
use embassy_embedded_hal::shared_bus::I2cDeviceError;
use embassy_time::{Duration, Instant, Timer};

use crate::{
    data::{Battery, Co2Data, Devices, PressureData, State, TemperatureData, VocData},
    measurements::voc::store_voc_state,
};

pub type I2cDevError = I2cDeviceError<esp_hal::i2c::master::Error>;

#[embassy_executor::task]
pub async fn measure(state: &'static State, devices: &'static Devices<'static>) {
    let mut last_co2_sample = Instant::now();
    loop {
        let refresh_secs = {
            let s = state.config.lock().await;
            s.update_interval
        };
        let beginning = Instant::now();
        let include_co2_sampling = beginning - last_co2_sample >= Duration::from_secs(5);
        if include_co2_sampling {
            last_co2_sample = beginning;
        }
        // info!("Include CO2: {}", include_co2_sampling);
        let d = measure_once(devices, include_co2_sampling).await;

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
        if include_co2_sampling {
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

pub async fn measure_once(
    devices: &'static Devices<'static>,
    include_co2: bool,
) -> MeasurementResult {
    let pressure = async {
        let reading: (f32, f32) = devices
            .icp
            .lock()
            .await
            .read_pressure_and_temperature()
            .await?;
        Ok::<_, async_icp20100::Error<I2cDevError>>(PressureData {
            pressure: reading.0,
            temperature: reading.1,
            error: false,
        })
    }
    .await
    .unwrap_or_else(|_| PressureData {
        error: true,
        ..PressureData::default()
    });
    let temperature = match devices.aht20.lock().await.measure().await {
        Ok(d) => TemperatureData {
            humidity: d.humidity,
            temperature: d.temperature,
            error: false,
        },
        Err(_) => TemperatureData {
            error: true,
            ..TemperatureData::default()
        },
    };
    let voc = async {
        if unsafe { crate::SGP40_ENABLED == 1 } {
            let mut sgp = devices.sgp40.lock().await;
            let reading = sgp
                .measure_voc_index_with_rht(
                    (temperature.humidity * 1000.0) as u16,
                    (temperature.temperature * 1000.0) as i16,
                )
                .await?;
            store_voc_state(&sgp.dump_algorithm_state());
            unsafe {
                crate::SGP40_READINGS += 1;
            }

            Ok::<_, sgp40::Error<I2cDevError>>(VocData {
                readings_until_warmup_complete: 0,
                value: reading as i32,
                error: false,
            })
        } else {
            Ok::<_, sgp40::Error<I2cDevError>>(VocData {
                readings_until_warmup_complete: 0,
                value: 0,
                error: false,
            })
        }
    }
    .await
    .unwrap_or_else(|_| VocData {
        error: true,
        ..VocData::default()
    });
    let co2 = match include_co2 {
        false => Co2Data::default(),
        true => async {
            let mut stcc4 = devices.stcc4.lock().await;
            stcc4
                .set_rht_compensation(temperature.temperature, temperature.humidity)
                .await?;
            stcc4
                .set_pressure_compensation((pressure.pressure * 1000.0) as i32)
                .await?;
            stcc4.single_shot(true).await?;
            let (co2, _, _) = stcc4.read_measurement().await?;
            Ok::<_, async_stcc4::Error<I2cDevError>>(Co2Data { co2, error: false })
        }
        .await
        .unwrap_or_else(|_| Co2Data {
            error: true,
            ..Co2Data::default()
        }),
    };
    let battery = async {
        let mut bq = devices.bq27441.lock().await;
        let avg_power = bq.average_power_mw().await?;
        // let avg_current = bq.avg_current_ma().await.unwrap();
        let voltage = bq.voltage_mv().await?;
        let soc = bq.soc_percent().await?;
        Ok::<_, bq27441::Error<I2cDevError>>(Battery {
            percentage: soc as i8,
            power: avg_power,
            voltage,
            error: false,
        })
    }
    .await
    .unwrap_or_else(|_| Battery {
        error: true,
        ..Battery::default()
    });
    MeasurementResult {
        co2,
        pressure,
        temperature,
        voc,
        battery,
    }
}
