pub mod long_write;
pub mod services;

use defmt::{Debug2Format, info, warn};
use embassy_futures::select::select3;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::signal::Signal;
use embassy_time::Timer;
use esp_radio::ble::controller::BleConnector;
use trouble_host::prelude::*;

use crate::bluetooth::long_write::{ConnectionContext, LongWriteAccumulator};
use crate::bluetooth::services::{MeasurementVec, Server};
use crate::data::Devices;
use crate::measurements::sampling::from_nvs;
use crate::{MEASUREMENT_SIGNAL, SAMPLES_PER_BUFFER};
use defmt::unwrap;
use embassy_futures::join::join;
/// Max number of connections
const CONNECTIONS_MAX: usize = 1;

pub static SAMPLE_PUBLISH_DATA: Signal<CriticalSectionRawMutex, u8> = Signal::new();

/// Max number of L2CAP channels.
const L2CAP_CHANNELS_MAX: usize = 2; // Signal + att
#[embassy_executor::task]
pub async fn run(
    controller: ExternalController<BleConnector<'static>, 20>,
    devices: &'static Devices<'static>,
) {
    // Using a fixed "random" address can be useful for testing. In real scenarios, one would
    // use e.g. the MAC 6 byte array as the address (how to get that varies by the platform).
    let address: Address = Address::random([0xff, 0x8f, 0x1a, 0x05, 0xe4, 0xff]);
    info!("Our address = {:?}", address);

    let mut resources: HostResources<DefaultPacketPool, CONNECTIONS_MAX, L2CAP_CHANNELS_MAX> =
        HostResources::new();
    let stack = trouble_host::new(controller, &mut resources).set_random_address(address);
    let Host {
        mut peripheral,
        runner,
        ..
    } = stack.build();

    info!("Starting advertising and GATT service");
    let server = Server::new_with_config(GapConfig::Peripheral(PeripheralConfig {
        name: "AmbientAir",
        appearance: &appearance::power_device::GENERIC_POWER_DEVICE,
    }))
    .unwrap();

    let _ = join(ble_task(runner), async {
        loop {
            match advertise("AmbientAir", &mut peripheral, &server).await {
                Ok(conn) => {
                    // set up tasks when the connection is established to a central, so they don't run when no one is connected.
                    let a = gatt_events_task(&server, &conn, devices);
                    let b = notify_task(&server, &conn, devices);
                    let c = notify_sampling_data(&server, &conn, devices);
                    // run until any task ends (usually because the connection has been closed),
                    // then return to advertising state.
                    select3(a, b, c).await;
                    // go_sleep_without_devices(crate::energy::sleep::SleepState::Standby).await;
                }
                Err(e) => {
                    let e = defmt::Debug2Format(&e);
                    panic!("[adv] error: {:?}", e);
                }
            }
        }
    })
    .await;
}

async fn ble_task<C: Controller, P: PacketPool>(mut runner: Runner<'_, C, P>) {
    loop {
        if let Err(e) = runner.run().await {
            let e = defmt::Debug2Format(&e);
            panic!("[ble_task] error: {:?}", e);
        }
    }
}

async fn gatt_events_task<P: PacketPool>(
    server: &Server<'_>,
    conn: &GattConnection<'_, '_, P>,
    devices: &'static Devices<'static>,
) -> Result<(), Error> {
    let mut ctx = ConnectionContext {
        long_write: LongWriteAccumulator::new(),
    };
    let reason = loop {
        match conn.next().await {
            GattConnectionEvent::Disconnected { reason } => break reason,
            GattConnectionEvent::Gatt { event } => {
                let long_write = match &event {
                    GattEvent::Other(e) => {
                        let acc = e.payload();
                        let inc = acc.incoming();
                        match inc {
                            trouble_host::att::AttClient::Request(req) => match req {
                                trouble_host::att::AttReq::PrepareWrite {
                                    handle,
                                    offset,
                                    value,
                                } => {
                                    let _ = ctx.long_write.prepare(handle, offset as usize, value);
                                    None
                                }
                                trouble_host::att::AttReq::ExecuteWrite { .. } => {
                                    Some(ctx.long_write.execute())
                                }
                                _ => None,
                            },
                            _ => None,
                        }
                    }
                    _ => None,
                };
                server.temperature.handle(&event, server, devices).await;
                server.pressure.handle(&event, server, devices).await;
                server.co2.handle(&event, server, devices).await;
                server.voc.handle(&event, server, devices).await;
                server
                    .measurement
                    .handle(&event, server, devices, long_write)
                    .await;
                server.battery.handle(&event, server, devices).await;
                server.time.handle(&event, server, devices).await;
                // This step is also performed at drop(), but writing it explicitly is necessary
                // in order to ensure reply is sent.
                if long_write.is_some() {
                    ctx.long_write.reset()
                }
                match event.accept() {
                    Ok(reply) => reply.send().await,
                    Err(e) => warn!("[gatt] error sending response: {:?}", e),
                };
            }
            _ => {} // ignore other Gatt Connection Events
        }
    };
    info!("[gatt] disconnected: {:?}", reason);
    Ok(())
}

/// Create an advertiser to use to connect to a BLE Central, and wait for it to connect.
async fn advertise<'values, 'server, C: Controller>(
    name: &'values str,
    peripheral: &mut Peripheral<'values, C, DefaultPacketPool>,
    server: &'server Server<'values>,
) -> Result<GattConnection<'values, 'server, DefaultPacketPool>, BleHostError<C::Error>> {
    let mut advertiser_data = [0; 31];
    let len = AdStructure::encode_slice(
        &[
            AdStructure::Flags(LE_GENERAL_DISCOVERABLE | BR_EDR_NOT_SUPPORTED),
            AdStructure::CompleteServiceUuids16(&[[0x0f, 0x18]]),
            AdStructure::CompleteLocalName(name.as_bytes()),
        ],
        &mut advertiser_data[..],
    )?;
    let advertiser = peripheral
        .advertise(
            &Default::default(),
            Advertisement::ConnectableScannableUndirected {
                adv_data: &advertiser_data[..len],
                scan_data: &[],
            },
        )
        .await?;
    info!("[adv] advertising");
    let conn = advertiser.accept().await?.with_attribute_server(server)?;
    info!("[adv] connection established");
    Ok(conn)
}

async fn notify_task<P: PacketPool>(
    server: &Server<'_>,
    conn: &GattConnection<'_, '_, P>,
    _devices: &'static Devices<'static>,
) -> Result<(), Error> {
    let mut recv = unwrap!(MEASUREMENT_SIGNAL.receiver());

    loop {
        let msg = recv.changed().await;
        server.battery.notify(conn, &msg.battery).await?;
        server.co2.notify(conn, &msg.co2).await?;
        server.pressure.notify(conn, &msg.pressure).await?;
        server.temperature.notify(conn, &msg.temperature).await?;
        server.voc.notify(conn, &msg.voc).await?;
    }
}

async fn notify_sampling_data<P: PacketPool>(
    server: &Server<'_>,
    conn: &GattConnection<'_, '_, P>,
    devices: &'static Devices<'static>,
) -> Result<(), Error> {
    loop {
        let _ = SAMPLE_PUBLISH_DATA.wait().await;
        info!("Sending measurement data");
        let nvs_chunks = unsafe { crate::MEASUREMENT_SAMPLES_REQUESTED }
            .div_ceil(crate::SAMPLES_PER_BUFFER as i16);
        info!("Got {} chunks", nvs_chunks);
        let notifys_needed = unsafe { crate::MEASUREMENT_SAMPLES_REQUESTED }.div_ceil(10) as usize;
        let mut notify_done = 0;

        'publish: for i in 0..nvs_chunks {
            let data = {
                let mut nvs = devices.nvs.lock().await;
                from_nvs(&mut nvs, i as usize).await
            };

            for n in 0..(SAMPLES_PER_BUFFER.div_ceil(10)) {
                if notify_done >= notifys_needed {
                    break 'publish;
                }
                let start = n * 10;
                let end = (n + 1) * 10;
                let d = match MeasurementVec::from_slice(&data[start..end]) {
                    Ok(d) => d,
                    Err(_) => {
                        warn!("MeasurementVec init failed");
                        continue;
                    }
                };
                info!("MMVec: {:?}", Debug2Format(&d));
                if d.is_empty() {
                    continue;
                }
                let r = server.measurement.data.notify(conn, &d).await;
                notify_done += 1;
                match r {
                    Ok(_) => (),
                    Err(e) => info!("{:?}", e),
                }
                Timer::after_millis(30).await;
            }
        }
    }
}
