use core::time::Duration;

use defmt::info;
use esp_hal::{
    peripherals,
    rtc_cntl::{Rtc, sleep::TimerWakeupSource},
};
use serde::{Deserialize, Serialize};
use trouble_host::{
    PacketPool,
    gatt::{GattEvent, ReadEvent},
    prelude::FromGatt,
};

use crate::{
    PowerState,
    bluetooth::{
        long_write::GenericWrite,
        services::{CommandBuf, MeasurementService, MeasurementVec, Server},
    },
    data::{Devices, State},
    handle_service,
    measurements::sampling::from_nvs,
};

impl MeasurementService {
    pub async fn handle<P: PacketPool>(
        &self,
        event: &GattEvent<'_, '_, P>,
        server: &Server<'_>,
        state: &'static State,
        devices: &'static Devices<'static>,
        long_write: Option<(&[u8], u16)>,
    ) {
        handle_service!(self, server, event, state, devices, long_write, {
            command => (read_command, write_command),
            data    => (read_data, write_data),
        });
    }
    pub async fn read_command<P: PacketPool>(
        &self,
        _e: &ReadEvent<'_, '_, P>,
        _server: &Server<'_>,
        _state: &'static State,
        _devices: &'static Devices<'static>,
    ) {
        unreachable!()
    }
    pub async fn read_data<P: PacketPool>(
        &self,
        _e: &ReadEvent<'_, '_, P>,
        server: &Server<'_>,
        _state: &'static State,
        devices: &'static Devices<'static>,
    ) {
        let data = {
            let mut nvs = devices.nvs.lock().await;
            from_nvs(&mut nvs, 0).await
        };
        server
            .measurement
            .data
            .set(server, &MeasurementVec(data))
            .unwrap();
    }

    pub async fn write_command<P: PacketPool>(
        &self,
        e: &GenericWrite<'_, CommandBuf>,
        _server: &Server<'_>,
        _state: &'static State,
        _devices: &'static Devices<'static>,
    ) {
        info!("Writing command");
        let val = match e {
            GenericWrite::Long { data, .. } => &CommandBuf::from_gatt(data).unwrap(),
            GenericWrite::Short(d) => d,
        };
        // converting to string to strip 0's out of oversized buffer
        let (d, ..) = serde_json_core::from_str::<MeasurementCommandData>(
            str::from_utf8(&val.0).unwrap().trim_end_matches('\0'),
        )
        .unwrap();
        unsafe {
            crate::MEASUREMENT_SAMPLES_TAKEN = 0;
            crate::MEASUREMENT_SAMPLES_REQUESTED = d.samples;
            crate::SAMPLE_EVERY_SECONDS = d.every_x_seconds;
            crate::POWER_STATE = PowerState::SampleMode as i8;
        }
        let mut rtc = Rtc::new(unsafe { peripherals::LPWR::steal() });
        rtc.sleep_deep(&[&TimerWakeupSource::new(Duration::from_millis(20))]);

        // info!("Value: {}", str_data)
    }

    pub async fn write_data<P: PacketPool>(
        &self,
        _e: &GenericWrite<'_, MeasurementVec>,
        _server: &Server<'_>,
        _state: &'static State,
        _devices: &'static Devices<'static>,
    ) {
        unreachable!()
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct MeasurementCommandData {
    every_x_seconds: i16,
    samples: i16,
}
