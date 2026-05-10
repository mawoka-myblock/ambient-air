pub mod battery;
pub mod co2;
pub mod measurement;
pub mod pressure;
pub mod temperature;
pub mod time;
pub mod voc;
use defmt::{error, info};
use heapless::{CapacityError, Vec};
use trouble_host::{prelude::*, types::gatt_traits::FromGattError};

use crate::measurements::sampling::{MEAS_SIZE, Measurement};

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
        if data.len() > Self::MAX_SIZE {
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

#[macro_export]
macro_rules! handle_service {
    (
        $svc:expr,
        $server:expr,
        $event:expr,
        $state_var:expr,
        $devices_var:expr,
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
                            $svc.$read_fn(e, $server, $state_var, $devices_var).await;
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
                            $svc.$write_fn::<P>(&gw, $server, $state_var, $devices_var).await;
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
                        $svc.$write_fn::<P>(&gw, $server, $state_var, $devices_var).await;
                    }
                )*
                _ => {}
            }
        }
    }};
}

#[gatt_server]
pub struct Server {
    pub battery: BatteryService,
    pub time: TimeService,
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
    #[characteristic(uuid = "408813df-5dd4-1f87-ec11-cdb001100000", read, notify)]
    pub power: i16,
    #[characteristic(uuid = "10bb24ab-a674-4630-b670-972eac8bf6cb", read, write)]
    pub capacity: i16,
}

#[gatt_service(uuid = "85083006-8da2-4d0b-9dca-fc3ccda46a3c")]
pub struct TimeService {
    #[characteristic(uuid = "9525ce8e-3d50-4975-a8e5-64ddea6dfe10", read, write)]
    pub time: u64, // in µs
}

#[gatt_service(uuid = "125ef8ff-f538-468f-9f40-2380a102895b")]
pub struct TemperatureService {
    #[descriptor(uuid = descriptors::MEASUREMENT_DESCRIPTION, name = "name", read, value = "Temperature")]
    #[characteristic(uuid = "561be71a-359d-4964-b64f-7b1c949b092e", read, notify)]
    pub temperature: f32,
    #[descriptor(uuid = descriptors::MEASUREMENT_DESCRIPTION, name = "name", read, value = "Humidity")]
    #[characteristic(uuid = "13881d03-54b9-4b8c-be9f-8a0eeec6893b", read, notify)]
    pub humidity: f32,
}

#[gatt_service(uuid = "5f78b426-c2dd-4c3f-864f-1b2ccdf1e63e")]
pub struct PressureService {
    #[descriptor(uuid = descriptors::MEASUREMENT_DESCRIPTION, name = "name", read, value = "Pressure")]
    #[characteristic(uuid = "7c4b9d53-cbce-409e-bb3d-06d7f9f263d8", read, notify)]
    pub pressure: f32,
    #[descriptor(uuid = descriptors::MEASUREMENT_DESCRIPTION, name = "name", read, value = "Temperature")]
    #[characteristic(uuid = "a3f6145d-d2eb-46a6-aa41-9644a44bb18e", read, notify)]
    pub temperature: f32,
}

#[gatt_service(uuid = "a6689992-6e99-4903-85ce-5750b7c4d995")]
pub struct Co2Service {
    #[descriptor(uuid = descriptors::MEASUREMENT_DESCRIPTION, name = "name", read, value = "CO2")]
    #[characteristic(uuid = "cfb04cf1-8d5b-4223-9ae5-c9e32b2940ab", read, notify)]
    pub co2: i16,
    #[characteristic(uuid = "22b0808a-3a60-45ed-9c54-57f1f16079e6", read, write)]
    pub sampling_interval: i16,
}

#[gatt_service(uuid = "9fdbefc6-0e57-469c-b006-8c38f517805a")]
pub struct VocService {
    #[descriptor(uuid = descriptors::MEASUREMENT_DESCRIPTION, name = "name", read, value = "VOC")]
    #[characteristic(uuid = "55697045-6e90-4940-b055-a03f9ae10122", read, notify)]
    pub index: i16,
    #[descriptor(uuid = descriptors::MEASUREMENT_DESCRIPTION, name = "name", read, value = "Reading Count")]
    #[characteristic(uuid = "93c32824-5d3b-4343-a5e9-5699d165bc47", read, notify)]
    pub count: i16,
    #[characteristic(uuid = "a1666baa-2fd2-456b-ab68-8e83395f9f79", read, write)]
    pub enabled: bool,
}

#[gatt_service(uuid = "aa830336-a632-4fb7-83c0-c3868760d858")]
pub struct MeasurementService {
    #[characteristic(uuid = "84b0a39c-c55f-41f3-8797-de87992adc55", write)]
    pub command: CommandBuf,
    #[characteristic(uuid = "d988b5cc-5154-45e2-9815-4d55261950ad", read)]
    pub sample_count: i16,
    #[characteristic(uuid = "127ec103-86ea-4e75-9e35-2e0c772d6f85", notify)]
    pub data: MeasurementVec,
}
#[derive(Debug, Default)]
pub struct MeasurementVec(pub Vec<Measurement, 10>);

impl MeasurementVec {
    pub fn from_slice(d: &[Measurement]) -> Result<Self, CapacityError> {
        let mut data: Vec<Measurement, 10> = Vec::new();
        info!("Src len: {}, dst len: {}", d.len(), data.len());
        data.extend_from_slice(d)?;
        Ok(MeasurementVec(data))
    }
}

impl AsGatt for MeasurementVec {
    const MAX_SIZE: usize = 304;
    const MIN_SIZE: usize = 0;
    fn as_gatt(&self) -> &[u8] {
        bytemuck::cast_slice(self.0.as_slice())
    }
}

impl FromGatt for MeasurementVec {
    fn from_gatt(data: &[u8]) -> Result<Self, FromGattError> {
        // Byte length must be a multiple of Measurement
        if data.len() % MEAS_SIZE != 0 {
            return Err(FromGattError::InvalidLength);
        }

        let measurements: &[Measurement] = bytemuck::cast_slice(data);

        let mut vec = MeasurementVec(Vec::new());

        for &m in measurements {
            vec.0.push(m).map_err(|_| FromGattError::InvalidLength)?;
        }

        Ok(vec)
    }
}
