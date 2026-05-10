use trouble_host::{
    PacketPool,
    gatt::{GattConnection, GattEvent, ReadEvent},
};

use crate::{
    bluetooth::{
        long_write::GenericWrite,
        services::{Server, VocService},
    },
    data::{Devices, State},
    handle_service,
};

impl VocService {
    pub async fn notify<P: PacketPool>(
        &self,
        conn: &GattConnection<'_, '_, P>,
        state: &State,
    ) -> Result<(), trouble_host::Error> {
        let (index, count) = {
            let s = state.voc.lock().await;
            (s.value, s.readings_until_warmup_complete)
        };
        self.index.notify(conn, &(index as i16)).await?;
        self.count.notify(conn, &(count as i16)).await?;
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
            index => (read_voc, write_voc),
            count    => (read_count, write_count),
            enabled => (read_enabled, write_enabled)
        });
    }
    pub async fn read_voc<P: PacketPool>(
        &self,
        _e: &ReadEvent<'_, '_, P>,
        server: &Server<'_>,
        state: &'static State,
        _devices: &'static Devices<'static>,
    ) {
        server
            .voc
            .index
            .set(server, &{
                let s = state.voc.lock().await;
                s.value as i16
            })
            .unwrap();
    }
    pub async fn read_count<P: PacketPool>(
        &self,
        _e: &ReadEvent<'_, '_, P>,
        server: &Server<'_>,
        state: &'static State,
        _devices: &'static Devices<'static>,
    ) {
        server
            .voc
            .count
            .set(server, &{
                let s = state.voc.lock().await;
                s.readings_until_warmup_complete as i16
            })
            .unwrap();
    }
    pub async fn read_enabled<P: PacketPool>(
        &self,
        _e: &ReadEvent<'_, '_, P>,
        server: &Server<'_>,
        _state: &'static State,
        devices: &'static Devices<'static>,
    ) {
        let sgp40_enabled = {
            let nvs = devices.nvs.lock().await;
            nvs.get_key(crate::nvs_keys::SGP40_ENABLED_KEY)
                .await
                .ok()
                .and_then(|d| d.0.first().copied())
                .map(|v| v != 0)
                .unwrap_or(false)
        };
        server.voc.enabled.set(server, &sgp40_enabled).unwrap();
    }

    pub async fn write_voc<P: PacketPool>(
        &self,
        _e: &GenericWrite<'_, i16>,
        _server: &Server<'_>,
        _state: &'static State,
        _devices: &'static Devices<'static>,
    ) {
        unreachable!()
    }

    pub async fn write_count<P: PacketPool>(
        &self,
        _e: &GenericWrite<'_, i16>,
        _server: &Server<'_>,
        _state: &'static State,
        _devices: &'static Devices<'static>,
    ) {
        unreachable!()
    }
    pub async fn write_enabled<P: PacketPool>(
        &self,
        e: &GenericWrite<'_, bool>,
        _server: &Server<'_>,
        _state: &'static State,
        devices: &'static Devices<'static>,
    ) {
        let data = match e {
            GenericWrite::Long { data: _, handle: _ } => false,
            GenericWrite::Short(d) => *d,
        };

        {
            let nvs = devices.nvs.lock().await;
            let _ = nvs.invalidate_key(crate::nvs_keys::SGP40_ENABLED_KEY).await;

            nvs.append_key(
                crate::nvs_keys::SGP40_ENABLED_KEY,
                if data { &[1] } else { &[0] },
            )
            .await
            .unwrap();
        }
        if !data {
            let mut sgp40 = devices.sgp40.lock().await;
            sgp40.turn_heater_off().await.unwrap();
        }

        unsafe { crate::SGP40_ENABLED = if data { 1 } else { 0 } }
    }
}
