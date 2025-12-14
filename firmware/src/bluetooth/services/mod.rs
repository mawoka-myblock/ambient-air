pub mod measurement;
use crate::bluetooth::long_write::GenericWrite;
use defmt::error;
use trouble_host::{prelude::*, types::gatt_traits::FromGattError};

use crate::data::State;
#[macro_export]
macro_rules! handle_service {
    (
        $svc:expr,
        $server:expr,
        $event:expr,
        $state_var:expr,
        $generic_write:expr,
        {
            $( $field:ident => ($read_fn:ident, $write_fn:ident) ),* $(,)?
        }
    ) => {{
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

            // Short write path
            GattEvent::Write(e) => {
                match e.handle() {
                    $(
                        h if h == $svc.$field.handle => {
                            let gw = GenericWrite::Short(e.value(&$svc.$field).unwrap());
                            $svc.$write_fn::<P>(&gw, $server, $state_var).await;
                        }
                    )*
                    _ => {}
                }
            }

            _ => {}
        }

        // Long write commit path (ExecuteWrite)
        if let Some((data, handle)) = $generic_write {
            match handle {
                $(
                    h if h == $svc.$field.handle => {
                        let gw = GenericWrite::Long { data, handle };
                        $svc.$write_fn::<P>(&gw, $server, $state_var).await;
                    }
                )*
                _ => {}
            }
        }
    }};
}

#[gatt_server]
pub struct Server {
    pub battery_service: BatteryService,
    pub temperature: TemperatureService,
    pub pressure: PressureService,
    pub co2: Co2Service,
    pub voc: VocService,
    pub measurement: MeasurementService,
    pub base: BaseData,
}

#[gatt_service(uuid = "1aba5096-5be2-4768-aef0-51c8667e1aa8")]
pub struct BaseData {}

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
        handle_service!(self, server, event, state, None, {
            temperature => (read_temperature, write_temperature),
            humidity    => (read_humidity, write_humidity),
        });
    }
    pub async fn read_temperature<P: PacketPool>(
        &self,
        _e: &ReadEvent<'_, '_, P>,
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
        _e: &ReadEvent<'_, '_, P>,
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
        _e: &GenericWrite<'_, f32>,
        _server: &Server<'_>,
        _state: &'static State,
    ) {
        unreachable!()
    }

    pub async fn write_humidity<P: PacketPool>(
        &self,
        _e: &GenericWrite<'_, f32>,
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
        handle_service!(self, server, event, state, None, {
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
        _e: &GenericWrite<'_, f32>,
        _server: &Server<'_>,
        _state: &'static State,
    ) {
        unreachable!()
    }

    pub async fn write_temperature<P: PacketPool>(
        &self,
        _e: &GenericWrite<'_, f32>,
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
        handle_service!(self, server, event, state, None, {
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
        _e: &GenericWrite<'_, i16>,
        _server: &Server<'_>,
        _state: &'static State,
    ) {
        unreachable!()
    }
}

#[gatt_service(uuid = "9fdbefc6-0e57-469c-b006-8c38f517805a")]
pub struct VocService {
    #[descriptor(uuid = descriptors::MEASUREMENT_DESCRIPTION, name = "name", read, value = "VOC")]
    #[characteristic(uuid = "55697045-6e90-4940-b055-a03f9ae10122", read)]
    pub index: i32,
    #[descriptor(uuid = descriptors::MEASUREMENT_DESCRIPTION, name = "name", read, value = "Reading Count")]
    #[characteristic(uuid = "93c32824-5d3b-4343-a5e9-5699d165bc47", read)]
    pub count: i32,
}
impl VocService {
    pub async fn handle<P: PacketPool>(
        &self,
        event: &GattEvent<'_, '_, P>,
        server: &Server<'_>,
        state: &'static State,
    ) {
        handle_service!(self, server, event, state, None, {
            index => (read_voc, write_voc),
            count    => (read_count, write_count),
        });
    }
    pub async fn read_voc<P: PacketPool>(
        &self,
        _e: &ReadEvent<'_, '_, P>,
        server: &Server<'_>,
        state: &'static State,
    ) {
        server
            .voc
            .index
            .set(server, &{
                let s = state.voc.lock().await;
                s.value
            })
            .unwrap();
    }
    pub async fn read_count<P: PacketPool>(
        &self,
        _e: &ReadEvent<'_, '_, P>,
        server: &Server<'_>,
        state: &'static State,
    ) {
        server
            .voc
            .count
            .set(server, &{
                let s = state.voc.lock().await;
                s.readings_until_warmup_complete
            })
            .unwrap();
    }

    pub async fn write_voc<P: PacketPool>(
        &self,
        _e: &GenericWrite<'_, i32>,
        _server: &Server<'_>,
        _state: &'static State,
    ) {
        unreachable!()
    }

    pub async fn write_count<P: PacketPool>(
        &self,
        _e: &GenericWrite<'_, i32>,
        _server: &Server<'_>,
        _state: &'static State,
    ) {
        unreachable!()
    }
}

#[derive(Copy, Clone)]
pub struct CommandBuf(pub [u8; 1024]);

impl Default for CommandBuf {
    fn default() -> Self {
        Self([0; 1024])
    }
}
impl AsGatt for CommandBuf {
    const MAX_SIZE: usize = 1024;
    const MIN_SIZE: usize = 0;
    fn as_gatt(&self) -> &[u8] {
        &self.0
    }
}
impl FromGatt for CommandBuf {
    fn from_gatt(data: &[u8]) -> Result<Self, FromGattError> {
        if data.len() < Self::MIN_SIZE || data.len() > Self::MAX_SIZE {
            error!("Invalid length");
            return Err(FromGattError::InvalidLength);
        }

        // Create a default buffer (all zeros)
        let mut buf = [0u8; Self::MAX_SIZE];

        // Copy the incoming data into the buffer
        buf[..data.len()].copy_from_slice(data);

        Ok(CommandBuf(buf))
    }
}

#[gatt_service(uuid = "aa830336-a632-4fb7-83c0-c3868760d858")]
pub struct MeasurementService {
    #[characteristic(uuid = "84b0a39c-c55f-41f3-8797-de87992adc55", write)]
    pub command: CommandBuf,
    #[characteristic(uuid = "127ec103-86ea-4e75-9e35-2e0c772d6f85", read)]
    pub data: heapless::String<4096>,
}
// impl AsGatt for &'static str {
//     const MIN_SIZE: usize = 0;
//     const MAX_SIZE: usize = usize::MAX;

//     fn as_gatt(&self) -> &[u8] {
//         self.as_bytes()
//     }
// }
