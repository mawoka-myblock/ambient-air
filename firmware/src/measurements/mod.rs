use embassy_time::{Duration, Instant, Timer};

use crate::data::{Devices, State};

#[embassy_executor::task]
pub async fn measure(state: &'static State, mut devices: Devices<'static>) {
    loop {
        let refresh_secs = {
            let s = state.config.lock().await;
            s.update_interval
        };
        let beginning = Instant::now();
        {
            let reading = devices.icp.read_pressure_and_temperature().await.unwrap();
            let mut s = state.pressure.lock().await;
            s.pressure = reading.0;
            s.temperature = reading.1;
        }
        let (humidity, temp) = {
            let reading = devices.aht20.measure().await.unwrap();
            let mut s = state.temperature.lock().await;
            s.humidity = reading.humidity;
            s.temperature = reading.temperature;
            (
                (reading.humidity * 1000.0) as u16,
                (reading.temperature * 1000.0) as i16,
            )
        };
        {
            let reading = devices
                .sgp40
                .measure_voc_index_with_rht(humidity, temp)
                .await
                .unwrap() as i32;
            let mut s = state.voc.lock().await;
            if s.readings_until_warmup_complete > 0 {
                s.readings_until_warmup_complete -= 1
            }
            s.value = reading
        };
        {
            devices.stcc4.single_shot().await.unwrap();
            let (co2, _, _) = devices.stcc4.read_measurement().await.unwrap();
            let mut s = state.co2.lock().await;
            s.co2 = co2;
        }
        // {
        //     let raw_data = devices.adc.read_oneshot(devices.adc_pin).await;
        //     let raw_voltage = raw_data as u32 * 2500 / 4095;
        //     let bat_voltage: f32 = raw_voltage as f32 * 2.2 / 1000.0;
        //     let mut s = state.battery.lock().await;
        //     s.charging = false;
        //     s.percentage = 50.0;
        //     s.voltage = bat_voltage;
        // }
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
        Timer::after(sleep_time).await;
    }
}
