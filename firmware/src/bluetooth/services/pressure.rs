use trouble_host::{
    PacketPool,
    gatt::{GattConnection, GattEvent, ReadEvent},
};

use crate::{
    bluetooth::{
        long_write::GenericWrite,
        services::{PressureService, Server},
    },
    data::{Devices, State},
    handle_service,
};

impl PressureService {
    pub async fn notify<P: PacketPool>(
        &self,
        conn: &GattConnection<'_, '_, P>,
        state: &State,
    ) -> Result<(), trouble_host::Error> {
        let (temp, pres) = {
            let s = state.pressure.lock().await;
            (s.temperature, s.pressure)
        };
        self.temperature.notify(conn, &temp).await?;
        self.pressure.notify(conn, &pres).await?;
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
            pressure => (read_pressure, write_pressure),
            temperature    => (read_temperature, write_temperature),
        });
    }
    pub async fn read_pressure<P: PacketPool>(
        &self,
        _e: &ReadEvent<'_, '_, P>,
        server: &Server<'_>,
        state: &'static State,
        _devices: &'static Devices<'static>,
    ) {
        server
            .pressure
            .pressure
            .set(server, &{
                let s = state.pressure.lock().await;
                s.pressure
            })
            .unwrap();
    }
    pub async fn read_temperature<P: PacketPool>(
        &self,
        _e: &ReadEvent<'_, '_, P>,
        server: &Server<'_>,
        state: &'static State,
        _devices: &'static Devices<'static>,
    ) {
        server
            .pressure
            .temperature
            .set(server, &{
                let s = state.pressure.lock().await;
                s.temperature
            })
            .unwrap();
    }

    pub async fn write_pressure<P: PacketPool>(
        &self,
        _e: &GenericWrite<'_, f32>,
        _server: &Server<'_>,
        _state: &'static State,
        _devices: &'static Devices<'static>,
    ) {
        unreachable!()
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
}
