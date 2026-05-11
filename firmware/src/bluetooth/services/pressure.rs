use trouble_host::{
    PacketPool,
    gatt::{GattConnection, GattEvent, ReadEvent},
};

use crate::{
    MEASUREMENT_SIGNAL,
    bluetooth::{
        long_write::GenericWrite,
        services::{PressureService, Server},
    },
    data::{Devices, PressureData},
    handle_service,
};

impl PressureService {
    pub async fn notify<P: PacketPool>(
        &self,
        conn: &GattConnection<'_, '_, P>,
        m: &PressureData,
    ) -> Result<(), trouble_host::Error> {
        self.temperature.notify(conn, &m.temperature).await?;
        self.pressure.notify(conn, &m.pressure).await?;
        Ok(())
    }
    pub async fn handle<P: PacketPool>(
        &self,
        event: &GattEvent<'_, '_, P>,
        server: &Server<'_>,
        devices: &'static Devices<'static>,
    ) {
        handle_service!(self, server, event, devices, None, {
            pressure => (read_pressure, write_pressure),
            temperature    => (read_temperature, write_temperature),
        });
    }
    pub async fn read_pressure<P: PacketPool>(
        &self,
        _e: &ReadEvent<'_, '_, P>,
        server: &Server<'_>,
        _devices: &'static Devices<'static>,
    ) {
        server
            .pressure
            .pressure
            .set(
                server,
                &MEASUREMENT_SIGNAL
                    .anon_receiver()
                    .try_get()
                    .unwrap()
                    .pressure
                    .pressure,
            )
            .unwrap();
    }
    pub async fn read_temperature<P: PacketPool>(
        &self,
        _e: &ReadEvent<'_, '_, P>,
        server: &Server<'_>,
        _devices: &'static Devices<'static>,
    ) {
        server
            .pressure
            .temperature
            .set(
                server,
                &MEASUREMENT_SIGNAL
                    .anon_receiver()
                    .try_get()
                    .unwrap()
                    .pressure
                    .temperature,
            )
            .unwrap();
    }

    pub async fn write_pressure<P: PacketPool>(
        &self,
        _e: &GenericWrite<'_, f32>,
        _server: &Server<'_>,
        _devices: &'static Devices<'static>,
    ) {
        unreachable!()
    }

    pub async fn write_temperature<P: PacketPool>(
        &self,
        _e: &GenericWrite<'_, f32>,
        _server: &Server<'_>,
        _devices: &'static Devices<'static>,
    ) {
        unreachable!()
    }
}
