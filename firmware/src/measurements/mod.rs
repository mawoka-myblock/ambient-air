pub mod lp;
pub mod sampling;
pub mod sensors;
pub mod voc;
use defmt::{Debug2Format, Format, debug, error};
use embassy_embedded_hal::shared_bus::I2cDeviceError;
use embassy_time::{Duration, Instant, Timer};
use esp_hal::{peripherals, rtc_cntl::Rtc, time::Duration as HalDuration};

use crate::{
    CONFIG_SIGNAL, MEASUREMENT_SIGNAL, SGP40_READINGS,
    data::{Battery, Co2Data, Devices, PressureData, TemperatureData, VocData},
    measurements::voc::store_voc_state,
    tasks::stcc4::{Stcc4State, get_stcc4_state},
};

pub type I2cDevError = I2cDeviceError<esp_hal::i2c::master::Error>;

#[embassy_executor::task]
pub async fn measure(devices: &'static Devices<'static>) {
    let mut last_co2_sample = Instant::now();
    let mm_signal = MEASUREMENT_SIGNAL.sender();
    let mut first_run = true;
    loop {
        let refresh_secs = match CONFIG_SIGNAL.anon_receiver().try_get() {
            Some(d) => d.update_interval,
            None => 1,
        };
        let beginning = Instant::now();
        let include_co2_sampling = beginning - last_co2_sample >= Duration::from_secs(5);
        if include_co2_sampling {
            last_co2_sample = beginning;
        }
        let mut d = measure_once(
            devices,
            include_co2_sampling,
            &Rtc::new(unsafe { peripherals::LPWR::steal() }).time_since_power_up(),
        )
        .await;
        debug!("{:?}", d);
        if first_run {
            d.voc.readings_until_warmup_complete = d
                .voc
                .readings_until_warmup_complete
                .saturating_sub(unsafe { SGP40_READINGS } as i32)
                .clamp(0, 50);
        }
        first_run = false;

        mm_signal.send(d);
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
#[derive(Debug, Clone, Copy, Format)]
pub struct MeasurementResult {
    pub pressure: PressureData,
    pub temperature: TemperatureData,
    pub voc: VocData,
    pub co2: Co2Data,
    pub battery: Battery,
}
/// Get single MeasurementResult
/// Needs `include_co2` to be set so it doesn't sample more than every 5 secs
pub async fn measure_once(
    devices: &'static Devices<'static>,
    include_co2: bool,
    dur_since_boot: &HalDuration,
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
    .unwrap_or_else(|e| {
        error!("Pressure: {:?}", Debug2Format(&e));
        PressureData {
            error: true,
            ..PressureData::default()
        }
    });
    let temperature = match devices.aht20.lock().await.measure().await {
        Ok(d) => TemperatureData {
            humidity: d.humidity,
            temperature: d.temperature,
            error: false,
        },
        Err(e) => {
            error!("CO2: {:?}", Debug2Format(&e));
            TemperatureData {
                error: true,
                ..TemperatureData::default()
            }
        }
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
    .unwrap_or_else(|e| {
        error!("VOC: {:?}", Debug2Format(&e));
        VocData {
            error: true,
            ..VocData::default()
        }
    });
    let co2 = match include_co2 {
        false => Co2Data::default(),
        true => async {
            let stcc4_state = get_stcc4_state(dur_since_boot);
            let mut stcc4 = devices.stcc4.lock().await;
            stcc4
                .set_rht_compensation(temperature.temperature, temperature.humidity)
                .await?;
            stcc4
                .set_pressure_compensation((pressure.pressure * 1000.0) as i32)
                .await?;
            match stcc4_state {
                Stcc4State::InContinous => (),
                Stcc4State::NeedsContinousStop => stcc4.stop_continuous().await?,
                Stcc4State::Normal => stcc4.single_shot(true).await?,
            };
            debug!("STCC4 State: {}", stcc4_state);
            let (co2, _, _) = stcc4.read_measurement().await?;
            Ok::<_, async_stcc4::Error<I2cDevError>>(Co2Data { co2, error: false })
        }
        .await
        .unwrap_or_else(|e| {
            error!("CO2: {:?}", Debug2Format(&e));
            Co2Data {
                error: true,
                ..Co2Data::default()
            }
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
    .unwrap_or_else(|e| {
        error!("Battery: {:?}", Debug2Format(&e));
        Battery {
            error: true,
            ..Battery::default()
        }
    });
    MeasurementResult {
        co2,
        pressure,
        temperature,
        voc,
        battery,
    }
}
