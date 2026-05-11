use esp_hal::{peripherals, rtc_cntl::Rtc};
use trouble_host::{
    PacketPool,
    gatt::{GattEvent, ReadEvent},
};

use crate::{
    bluetooth::{
        long_write::GenericWrite,
        services::{Server, TimeService},
    },
    data::Devices,
    handle_service,
};

impl TimeService {
    pub async fn handle<P: PacketPool>(
        &self,
        event: &GattEvent<'_, '_, P>,
        server: &Server<'_>,
        devices: &'static Devices<'static>,
    ) {
        handle_service!(self, server, event, devices, None, {
            time => (read_time, write_time),
        });
    }
    pub async fn read_time<P: PacketPool>(
        &self,
        _e: &ReadEvent<'_, '_, P>,
        server: &Server<'_>,
        _devices: &'static Devices<'static>,
    ) {
        let rtc = Rtc::new(unsafe { peripherals::LPWR::steal() });
        server
            .time
            .time
            .set(server, &rtc.current_time_us())
            .unwrap();
    }

    pub async fn write_time<P: PacketPool>(
        &self,
        e: &GenericWrite<'_, u64>,
        _server: &Server<'_>,
        _devices: &'static Devices<'static>,
    ) {
        let data = match e {
            GenericWrite::Long { data: _, handle: _ } => 0,
            GenericWrite::Short(d) => *d,
        };
        let rtc = Rtc::new(unsafe { peripherals::LPWR::steal() });
        rtc.set_current_time_us(data);
    }
}
