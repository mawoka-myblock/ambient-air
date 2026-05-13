use defmt::info;
use trouble_host::{
    PacketPool,
    gatt::{GattConnection, GattEvent, ReadEvent},
};

use crate::{
    MEASUREMENT_SIGNAL,
    bluetooth::{
        long_write::GenericWrite,
        services::{Co2Service, Server},
    },
    data::{Co2Data, Devices},
    handle_service,
};

impl Co2Service {
    pub async fn notify<P: PacketPool>(
        &self,
        conn: &GattConnection<'_, '_, P>,
        m: &Co2Data,
    ) -> Result<(), trouble_host::Error> {
        if m.co2 == 0 {
            return Ok(());
        }
        self.co2.notify(conn, &m.co2).await?;
        Ok(())
    }

    pub async fn handle<P: PacketPool>(
        &self,
        event: &GattEvent<'_, '_, P>,
        server: &Server<'_>,
        devices: &'static Devices<'static>,
    ) {
        handle_service!(self, server, event, devices, None, {
            co2 => (read_co2, write_co2),
            sampling_interval => (read_sampling_interval, write_sampling_interval),
        });
    }
    pub async fn read_co2<P: PacketPool>(
        &self,
        _e: &ReadEvent<'_, '_, P>,
        server: &Server<'_>,
        _devices: &'static Devices<'static>,
    ) {
        server
            .co2
            .co2
            .set(
                server,
                &MEASUREMENT_SIGNAL
                    .anon_receiver()
                    .try_get()
                    .unwrap()
                    .co2
                    .co2,
            )
            .unwrap();
    }

    pub async fn write_co2<P: PacketPool>(
        &self,
        _e: &GenericWrite<'_, i16>,
        _server: &Server<'_>,
        _devices: &'static Devices<'static>,
    ) {
        unreachable!()
    }
    pub async fn read_sampling_interval<P: PacketPool>(
        &self,
        _e: &ReadEvent<'_, '_, P>,
        server: &Server<'_>,
        _devices: &'static Devices<'static>,
    ) {
        server
            .co2
            .sampling_interval
            .set(server, &unsafe { crate::STCC4_SAMPLE_RATE })
            .unwrap();
    }

    pub async fn write_sampling_interval<P: PacketPool>(
        &self,
        e: &GenericWrite<'_, i16>,
        _server: &Server<'_>,
        _devices: &'static Devices<'static>,
    ) {
        let data = match e {
            GenericWrite::Long { data: _, handle: _ } => 600,
            GenericWrite::Short(d) => *d,
        };
        unsafe { crate::STCC4_SAMPLE_RATE = data.max(5) }
    }
}
