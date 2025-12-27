use trouble_host::{
    PacketPool,
    gatt::{GattConnection, GattEvent, ReadEvent},
};

use crate::{
    bluetooth::{
        long_write::GenericWrite,
        services::{Server, TemperatureService},
    },
    data::{Devices, State},
    handle_service,
};

impl TemperatureService {
    pub async fn notify<P: PacketPool>(
        &self,
        conn: &GattConnection<'_, '_, P>,
        state: &State,
    ) -> Result<(), trouble_host::Error> {
        let (temp, hum) = {
            let s = state.temperature.lock().await;
            (s.temperature, s.humidity)
        };
        self.temperature.notify(conn, &temp).await?;
        self.humidity.notify(conn, &hum).await?;
        Ok(())
    }

    pub async fn handle<P: PacketPool>(
        &self,
        event: &GattEvent<'_, '_, P>,
        server: &Server<'_>,
        state: &'static State,
        devices: &'static Devices<'static>,
    ) {
        handle_service!(self, server, event, state, devices, None, {
            temperature => (read_temperature, write_temperature),
            humidity    => (read_humidity, write_humidity),
        });
    }
    pub async fn read_temperature<P: PacketPool>(
        &self,
        _e: &ReadEvent<'_, '_, P>,
        server: &Server<'_>,
        state: &'static State,
        _devices: &'static Devices<'static>,
    ) {
        server
            .temperature
            .temperature
            .set(server, &{
                let s = state.temperature.lock().await;
                s.temperature
            })
            .unwrap();
    }
    pub async fn read_humidity<P: PacketPool>(
        &self,
        _e: &ReadEvent<'_, '_, P>,
        server: &Server<'_>,
        state: &'static State,
        _devices: &'static Devices<'static>,
    ) {
        server
            .temperature
            .humidity
            .set(server, &{
                let s = state.temperature.lock().await;
                s.humidity
            })
            .unwrap();
    }

    pub async fn write_temperature<P: PacketPool>(
        &self,
        _e: &GenericWrite<'_, f32>,
        _server: &Server<'_>,
        _state: &'static State,
        _devices: &'static Devices<'static>,
    ) {
        unreachable!()
    }

    pub async fn write_humidity<P: PacketPool>(
        &self,
        _e: &GenericWrite<'_, f32>,
        _server: &Server<'_>,
        _state: &'static State,
        _devices: &'static Devices<'static>,
    ) {
        unreachable!()
    }
}
