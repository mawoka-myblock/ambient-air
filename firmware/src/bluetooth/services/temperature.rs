use trouble_host::{
    PacketPool,
    gatt::{GattConnection, GattEvent, ReadEvent},
};

use crate::{
    MEASUREMENT_SIGNAL,
    bluetooth::{
        long_write::GenericWrite,
        services::{Server, TemperatureService},
    },
    data::{Devices, TemperatureData},
    handle_service,
};

impl TemperatureService {
    pub async fn notify<P: PacketPool>(
        &self,
        conn: &GattConnection<'_, '_, P>,
        m: &TemperatureData,
    ) -> Result<(), trouble_host::Error> {
        self.temperature.notify(conn, &m.temperature).await?;
        self.humidity.notify(conn, &m.humidity).await?;
        Ok(())
    }

    pub async fn handle<P: PacketPool>(
        &self,
        event: &GattEvent<'_, '_, P>,
        server: &Server<'_>,
        devices: &'static Devices<'static>,
    ) {
        handle_service!(self, server, event, devices, None, {
            temperature => (read_temperature, write_temperature),
            humidity    => (read_humidity, write_humidity),
        });
    }
    pub async fn read_temperature<P: PacketPool>(
        &self,
        _e: &ReadEvent<'_, '_, P>,
        server: &Server<'_>,
        _devices: &'static Devices<'static>,
    ) {
        server
            .temperature
            .temperature
            .set(
                server,
                &MEASUREMENT_SIGNAL
                    .anon_receiver()
                    .try_get()
                    .unwrap()
                    .temperature
                    .temperature,
            )
            .unwrap();
    }
    pub async fn read_humidity<P: PacketPool>(
        &self,
        _e: &ReadEvent<'_, '_, P>,
        server: &Server<'_>,
        _devices: &'static Devices<'static>,
    ) {
        server
            .temperature
            .humidity
            .set(
                server,
                &MEASUREMENT_SIGNAL
                    .anon_receiver()
                    .try_get()
                    .unwrap()
                    .temperature
                    .humidity,
            )
            .unwrap();
    }

    pub async fn write_temperature<P: PacketPool>(
        &self,
        _e: &GenericWrite<'_, f32>,
        _server: &Server<'_>,
        _devices: &'static Devices<'static>,
    ) {
        unreachable!()
    }

    pub async fn write_humidity<P: PacketPool>(
        &self,
        _e: &GenericWrite<'_, f32>,
        _server: &Server<'_>,
        _devices: &'static Devices<'static>,
    ) {
        unreachable!()
    }
}
