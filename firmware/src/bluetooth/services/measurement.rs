use core::{str::FromStr, time::Duration};

use defmt::info;
use esp_hal::{
    peripherals,
    rtc_cntl::{Rtc, sleep::TimerWakeupSource},
};
use heapless::string::{OwnedStorage, StringInner};
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
        services::{CommandBuf, MeasurementService, Server},
    },
    data::{Devices, State},
    handle_service,
    measurements::sampling::from_nvs,
    storage::Nvs,
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
        _devices: &'static Devices<'static>,
    ) {
        let mut nvs = Nvs::new(crate::NVS_OFFSET, crate::NVS_SIZE, unsafe {
            peripherals::FLASH::steal()
        })
        .unwrap();
        let data = from_nvs(&mut nvs, 0).await;
        let str_data = serde_json_core::to_string::<_, 4096>(&data).unwrap();
        let d_test: StringInner<usize, OwnedStorage<4096>> =
            StringInner::from_str(str_data.as_str()).unwrap();
        server.measurement.data.set(server, &d_test).unwrap();
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
        _e: &GenericWrite<'_, heapless::String<4096>>,
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
