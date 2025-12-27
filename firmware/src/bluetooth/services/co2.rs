use trouble_host::{
    PacketPool,
    gatt::{GattConnection, GattEvent, ReadEvent},
};

use crate::{
    bluetooth::{
        long_write::GenericWrite,
        services::{Co2Service, Server},
    },
    data::{Devices, State},
    handle_service,
};

impl Co2Service {
    pub async fn notify<P: PacketPool>(
        &self,
        conn: &GattConnection<'_, '_, P>,
        state: &State,
    ) -> Result<(), trouble_host::Error> {
        let co2 = {
            let s = state.co2.lock().await;
            s.co2
        };
        self.co2.notify(conn, &co2).await?;
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
            co2 => (read_co2, write_co2),
            sampling_interval => (read_sampling_interval, write_sampling_interval),
        });
    }
    pub async fn read_co2<P: PacketPool>(
        &self,
        _e: &ReadEvent<'_, '_, P>,
        server: &Server<'_>,
        state: &'static State,
        _devices: &'static Devices<'static>,
    ) {
        server
            .co2
            .co2
            .set(server, &{
                let s = state.co2.lock().await;
                s.co2
            })
            .unwrap();
    }

    pub async fn write_co2<P: PacketPool>(
        &self,
        _e: &GenericWrite<'_, i16>,
        _server: &Server<'_>,
        _state: &'static State,
        _devices: &'static Devices<'static>,
    ) {
        unreachable!()
    }
    pub async fn read_sampling_interval<P: PacketPool>(
        &self,
        _e: &ReadEvent<'_, '_, P>,
        _server: &Server<'_>,
        _state: &'static State,
        _devices: &'static Devices<'static>,
    ) {
        todo!()
    }

    pub async fn write_sampling_interval<P: PacketPool>(
        &self,
        _e: &GenericWrite<'_, i16>,
        _server: &Server<'_>,
        _state: &'static State,
        _devices: &'static Devices<'static>,
    ) {
        todo!()
    }
}
