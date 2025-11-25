use trouble_host::prelude::*;

use crate::data::State;

macro_rules! handle_service {
    ($svc:expr, $server:expr, $event:expr, $state_var:expr, {
           $( $field:ident => ($read_fn:ident, $write_fn:ident) ),* $(,)?
       }) => {{
        match $event {
            GattEvent::Read(e) => {
                match e.handle() {
                    $(
                        h if h == $svc.$field.handle => {
                            $svc.$read_fn(e, $server, $state_var).await;
                        }
                    )*
                    _ => {}
                }
            }

            GattEvent::Write(e) => {
                match e.handle() {
                    $(
                        h if h == $svc.$field.handle => {
                            $svc.$write_fn(e, $server, $state_var).await;
                        }
                    )*
                    _ => {}
                }
            }

            _ => {}
        }
    }};
}

#[gatt_server]
pub struct Server {
    pub battery_service: BatteryService,
    pub temperature: TemperatureService,
    pub pressure: PressureService,
    pub co2: Co2Service,
}

#[gatt_service(uuid = service::BATTERY)]
pub struct BatteryService {
    #[descriptor(uuid = descriptors::VALID_RANGE, read, value = [0,100])]
    #[descriptor(uuid = descriptors::MEASUREMENT_DESCRIPTION, name = "hello", read, value = "Battery Level")]
    #[characteristic(uuid = characteristic::BATTERY_LEVEL, read, notify, value = 10)]
    pub level: u8,
    #[characteristic(uuid = "408813df-5dd4-1f87-ec11-cdb001100000", write, read, notify)]
    pub charging: bool,
}

#[gatt_service(uuid = "125ef8ff-f538-468f-9f40-2380a102895b")]
pub struct TemperatureService {
    #[descriptor(uuid = descriptors::MEASUREMENT_DESCRIPTION, name = "name", read, value = "Temperature")]
    #[characteristic(uuid = "561be71a-359d-4964-b64f-7b1c949b092e", read)]
    pub temperature: f32,
    #[descriptor(uuid = descriptors::MEASUREMENT_DESCRIPTION, name = "name", read, value = "Humidity")]
    #[characteristic(uuid = "13881d03-54b9-4b8c-be9f-8a0eeec6893b", read)]
    pub humidity: f32,
}
impl TemperatureService {
    pub async fn handle<P: PacketPool>(
        &self,
        event: &GattEvent<'_, '_, P>,
        server: &Server<'_>,
        state: &'static State,
    ) {
        handle_service!(self, server, event, state, {
            temperature => (read_temperature, write_temperature),
            humidity    => (read_humidity, write_humidity),
        });
    }
    pub async fn read_temperature<P: PacketPool>(
        &self,
        e: &ReadEvent<'_, '_, P>,
        server: &Server<'_>,
        state: &'static State,
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
        e: &ReadEvent<'_, '_, P>,
        server: &Server<'_>,
        state: &'static State,
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
        _e: &WriteEvent<'_, '_, P>,
        _server: &Server<'_>,
        _state: &'static State,
    ) {
        unreachable!()
    }

    pub async fn write_humidity<P: PacketPool>(
        &self,
        _e: &WriteEvent<'_, '_, P>,
        _server: &Server<'_>,
        _state: &'static State,
    ) {
        unreachable!()
    }
}

#[gatt_service(uuid = "5f78b426-c2dd-4c3f-864f-1b2ccdf1e63e")]
pub struct PressureService {
    #[descriptor(uuid = descriptors::MEASUREMENT_DESCRIPTION, name = "name", read, value = "Pressure")]
    #[characteristic(uuid = "7c4b9d53-cbce-409e-bb3d-06d7f9f263d8", read)]
    pub pressure: f32,
    #[descriptor(uuid = descriptors::MEASUREMENT_DESCRIPTION, name = "name", read, value = "Temperature")]
    #[characteristic(uuid = "a3f6145d-d2eb-46a6-aa41-9644a44bb18e", read)]
    pub temperature: f32,
}
impl PressureService {
    pub async fn handle<P: PacketPool>(
        &self,
        event: &GattEvent<'_, '_, P>,
        server: &Server<'_>,
        state: &'static State,
    ) {
        handle_service!(self, server, event, state, {
            pressure => (read_pressure, write_pressure),
            temperature    => (read_temperature, write_temperature),
        });
    }
    pub async fn read_pressure<P: PacketPool>(
        &self,
        _e: &ReadEvent<'_, '_, P>,
        server: &Server<'_>,
        state: &'static State,
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
        _e: &WriteEvent<'_, '_, P>,
        _server: &Server<'_>,
        _state: &'static State,
    ) {
        unreachable!()
    }

    pub async fn write_temperature<P: PacketPool>(
        &self,
        _e: &WriteEvent<'_, '_, P>,
        _server: &Server<'_>,
        _state: &'static State,
    ) {
        unreachable!()
    }
}

#[gatt_service(uuid = "a6689992-6e99-4903-85ce-5750b7c4d995")]
pub struct Co2Service {
    #[descriptor(uuid = descriptors::MEASUREMENT_DESCRIPTION, name = "name", read, value = "CO2")]
    #[characteristic(uuid = "cfb04cf1-8d5b-4223-9ae5-c9e32b2940ab", read)]
    pub co2: i16,
}
impl Co2Service {
    pub async fn handle<P: PacketPool>(
        &self,
        event: &GattEvent<'_, '_, P>,
        server: &Server<'_>,
        state: &'static State,
    ) {
        handle_service!(self, server, event, state, {
            co2 => (read_co2, write_co2),
        });
    }
    pub async fn read_co2<P: PacketPool>(
        &self,
        _e: &ReadEvent<'_, '_, P>,
        server: &Server<'_>,
        state: &'static State,
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
        _e: &WriteEvent<'_, '_, P>,
        _server: &Server<'_>,
        _state: &'static State,
    ) {
        unreachable!()
    }
}
