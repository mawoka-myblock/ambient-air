use trouble_host::{
    PacketPool,
    gatt::{GattConnection, GattEvent, ReadEvent},
};

use crate::{
    bluetooth::{
        long_write::GenericWrite,
        services::{BatteryService, Server},
    },
    data::{Devices, State},
    handle_service,
};

impl BatteryService {
    pub async fn notify<P: PacketPool>(
        &self,
        conn: &GattConnection<'_, '_, P>,
        state: &State,
    ) -> Result<(), trouble_host::Error> {
        let (power, level) = {
            let s = state.battery.lock().await;
            (s.power, s.percentage)
        };
        self.level.notify(conn, &(level as u8)).await?;
        self.power.notify(conn, &power).await?;
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
            level => (read_level, write_level),
            power    => (read_power, write_power),
            capacity => (read_capacity, write_capacity)
        });
    }
    pub async fn read_level<P: PacketPool>(
        &self,
        _e: &ReadEvent<'_, '_, P>,
        server: &Server<'_>,
        state: &'static State,
        _devices: &'static Devices<'static>,
    ) {
        server
            .battery
            .level
            .set(server, &{
                let s = state.battery.lock().await;
                s.percentage as u8
            })
            .unwrap();
    }
    pub async fn read_power<P: PacketPool>(
        &self,
        _e: &ReadEvent<'_, '_, P>,
        server: &Server<'_>,
        state: &'static State,
        _devices: &'static Devices<'static>,
    ) {
        server
            .battery
            .power
            .set(server, &{
                let s = state.battery.lock().await;
                s.power
            })
            .unwrap();
    }
    pub async fn read_capacity<P: PacketPool>(
        &self,
        _e: &ReadEvent<'_, '_, P>,
        server: &Server<'_>,
        _state: &'static State,
        devices: &'static Devices<'static>,
    ) {
        server
            .battery
            .capacity
            .set(server, &{
                let mut bq = devices.bq27441.lock().await;
                bq.design_capacity_mah().await.unwrap() as i16
            })
            .unwrap();
    }

    pub async fn write_level<P: PacketPool>(
        &self,
        _e: &GenericWrite<'_, u8>,
        _server: &Server<'_>,
        _state: &'static State,
        _devices: &'static Devices<'static>,
    ) {
        unreachable!()
    }

    pub async fn write_power<P: PacketPool>(
        &self,
        _e: &GenericWrite<'_, i16>,
        _server: &Server<'_>,
        _state: &'static State,
        _devices: &'static Devices<'static>,
    ) {
        unreachable!()
    }

    pub async fn write_capacity<P: PacketPool>(
        &self,
        e: &GenericWrite<'_, i16>,
        _server: &Server<'_>,
        _state: &'static State,
        devices: &'static Devices<'static>,
    ) {
        let data = match e {
            GenericWrite::Long { data: _, handle: _ } => 500,
            GenericWrite::Short(d) => *d,
        };
        {
            let mut bq = devices.bq27441.lock().await;
            bq.set_design_capacity(data as u16).await.unwrap();
        }
    }
}
