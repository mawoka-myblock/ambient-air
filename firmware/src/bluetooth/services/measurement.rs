use defmt::{Debug2Format, Display2Format, info};
use embassy_time::Timer;
use serde::{Deserialize, Serialize};
use trouble_host::{
    PacketPool,
    gatt::{GattEvent, ReadEvent},
    prelude::FromGatt,
};

use crate::{
    COMMAND_CHANNEL, Commands, PowerState, SleepOptions,
    bluetooth::{
        SAMPLE_PUBLISH_DATA,
        long_write::GenericWrite,
        services::{CommandBuf, MeasurementService, Server},
    },
    data::Devices,
    handle_service,
};

impl MeasurementService {
    pub async fn handle<P: PacketPool>(
        &self,
        event: &GattEvent<'_, '_, P>,
        server: &Server<'_>,
        devices: &'static Devices<'static>,
        long_write: Option<(&[u8], u16)>,
    ) {
        if let GattEvent::Write(d) = event {
            info!("{:?}", d.handle());
        }
        handle_service!(self, server, event, devices, long_write, {
            command => (read_command, write_command),
            sample_count    => (read_sample_count, write_sample_count),
        });
    }
    pub async fn read_command<P: PacketPool>(
        &self,
        _e: &ReadEvent<'_, '_, P>,
        _server: &Server<'_>,
        _devices: &'static Devices<'static>,
    ) {
        unreachable!()
    }
    pub async fn read_sample_count<P: PacketPool>(
        &self,
        _e: &ReadEvent<'_, '_, P>,
        server: &Server<'_>,
        _devices: &'static Devices<'static>,
    ) {
        info!("recvd sample count");
        server
            .measurement
            .sample_count
            .set(server, &unsafe { crate::MEASUREMENT_SAMPLES_REQUESTED })
            .unwrap();
        Timer::after_millis(100).await;
        SAMPLE_PUBLISH_DATA.signal(0x00);
    }

    pub async fn write_command<P: PacketPool>(
        &self,
        e: &GenericWrite<'_, CommandBuf>,
        _server: &Server<'_>,
        _devices: &'static Devices<'static>,
    ) {
        info!("Received write");
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
        COMMAND_CHANNEL
            .immediate_publisher()
            .publish_immediate(Commands::Sleep(SleepOptions {
                wake_in_ms: Some(20),
                allow_buttons: true,
            }));
        info!("Sending sleep command");
    }

    pub async fn write_sample_count<P: PacketPool>(
        &self,
        _e: &GenericWrite<'_, i16>,
        _server: &Server<'_>,
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
