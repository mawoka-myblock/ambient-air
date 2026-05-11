use trouble_host::{
    PacketPool,
    gatt::{GattConnection, GattEvent, ReadEvent},
};

use crate::{
    MEASUREMENT_SIGNAL,
    bluetooth::{
        long_write::GenericWrite,
        services::{BatteryService, Server},
    },
    data::{Battery, Devices},
    handle_service,
};

impl BatteryService {
    pub async fn notify<P: PacketPool>(
        &self,
        conn: &GattConnection<'_, '_, P>,
        m: &Battery,
    ) -> Result<(), trouble_host::Error> {
        self.level.notify(conn, &(m.percentage as u8)).await?;
        self.power.notify(conn, &m.power).await?;
        Ok(())
    }

    pub async fn handle<P: PacketPool>(
        &self,
        event: &GattEvent<'_, '_, P>,
        server: &Server<'_>,
        devices: &'static Devices<'static>,
    ) {
        handle_service!(self, server, event, devices, None, {
            level => (read_level, write_level),
            power    => (read_power, write_power),
            capacity => (read_capacity, write_capacity)
        });
    }
    pub async fn read_level<P: PacketPool>(
        &self,
        _e: &ReadEvent<'_, '_, P>,
        server: &Server<'_>,
        _devices: &'static Devices<'static>,
    ) {
        server
            .battery
            .level
            .set(server, &{
                MEASUREMENT_SIGNAL
                    .anon_receiver()
                    .try_get()
                    .unwrap()
                    .battery
                    .percentage as u8
            })
            .unwrap();
    }
    pub async fn read_power<P: PacketPool>(
        &self,
        _e: &ReadEvent<'_, '_, P>,
        server: &Server<'_>,
        _devices: &'static Devices<'static>,
    ) {
        server
            .battery
            .power
            .set(server, &{
                MEASUREMENT_SIGNAL
                    .anon_receiver()
                    .try_get()
                    .unwrap()
                    .battery
                    .power
            })
            .unwrap();
    }
    pub async fn read_capacity<P: PacketPool>(
        &self,
        _e: &ReadEvent<'_, '_, P>,
        server: &Server<'_>,
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
        _devices: &'static Devices<'static>,
    ) {
        unreachable!()
    }

    pub async fn write_power<P: PacketPool>(
        &self,
        _e: &GenericWrite<'_, i16>,
        _server: &Server<'_>,
        _devices: &'static Devices<'static>,
    ) {
        unreachable!()
    }

    pub async fn write_capacity<P: PacketPool>(
        &self,
        e: &GenericWrite<'_, i16>,
        _server: &Server<'_>,
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
