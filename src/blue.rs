use embassy_futures::yield_now;
use embassy_sync::blocking_mutex::raw::RawMutex;
use embassy_sync::channel::Sender;
use log::error;
use log::info;

use embassy_futures::join::join3;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_time::{Duration, Timer};
use trouble_host::prelude::*;

use crate::Color;
use crate::lighting::Message;

/// Size of L2CAP packets (ATT MTU is this - 4)
const L2CAP_MTU: usize = 251;

/// Max number of connections
const CONNECTIONS_MAX: usize = 1;

/// Max number of L2CAP channels.
const L2CAP_CHANNELS_MAX: usize = 2; // Signal + att

const MAX_ATTRIBUTES: usize = 32;

type Resources<C> = HostResources<C, CONNECTIONS_MAX, L2CAP_CHANNELS_MAX, L2CAP_MTU>;

// GATT Server definition
#[gatt_server(attribute_data_size = 32)]
struct Server {
    mansion_lighting: MansionLighting
}

type C = [u8; 3];
#[gatt_service(uuid = "6D69636861656C73206D616E73696F6E")]
struct MansionLighting {
    #[characteristic(uuid = "42617365436F6C6F7200000000000000", write)]
    base_color: C,
    #[characteristic(uuid = "4272696768746E6573730A0000000000", write)]
    brightness: u8,
    #[characteristic(uuid = "536B6970000000000000000000000000", write)]
    skip: u8,
}

pub async fn run<C: Controller, M: RawMutex, const N: usize>(
    controller: C, 
    sender: Sender<'_, M, Message, N>
) {
    let address = Address::random([0xff, 0x9f, 0x1a, 0x05, 0xe4, 0xff]);
    info!("Our address = {:?}", address);

    let mut resources = Resources::new(PacketQos::None);
    let (stack, peripheral, _, runner) = trouble_host::new(controller, &mut resources)
        .set_random_address(address)
        .build();

    let mut table: AttributeTable<'_, NoopRawMutex, MAX_ATTRIBUTES> = AttributeTable::new();

    // Generic Access Service (mandatory)
    let id = b"mansion lighting";
    let appearance = [0x80, 0x07];
    let mut svc = table.add_service(Service::new(0x1800));
    let _ = svc.add_characteristic_ro(0x2a00, id);
    let _ = svc.add_characteristic_ro(0x2a01, &appearance[..]);
    svc.build();

    // Generic attribute service (mandatory)
    table.add_service(Service::new(0x1801));

    let server = Server::new(stack, &mut table);

    info!("Starting advertising and GATT service");
    let _ = join3(
        ble_task(runner),
        gatt_task(&server, sender),
        advertise_task(peripheral, &server),
    )
    .await;
}

async fn ble_task<C: Controller>(mut runner: Runner<'_, C>) {
    if let Err(e) = runner.run().await {
        error!("ble_task ERROR: {e:?}");
    }

    // we call sys_reset here because the bluetooth
    // stack can't handle reconnections for whatever reason.
    // TODO: use a better way to reset the bluetooth stack
    cortex_m::peripheral::SCB::sys_reset();
}

async fn gatt_task<C: Controller, M: RawMutex, const N: usize>(
    server: &Server<'_, '_, C>,
    sender: Sender<'_, M, Message, N>
) {
    loop {
        match server.next().await {
            Ok(GattEvent::Write { handle, connection: _ }) => {
                info!("[gatt] pre write event on {:?}", handle);

                if handle == server.mansion_lighting.base_color {
                    info!("setting base color");
                    server.get(server.mansion_lighting.base_color, |value| {
                        let color = Color::new(value[0], value[1], value[2]);
                        sender.send(Message::SetColor(color))
                    }).unwrap().await;
                } else if handle == server.mansion_lighting.brightness {
                    info!("setting brightness");
                    server.get(server.mansion_lighting.brightness, |value| {
                        sender.send(Message::SetBrightness(value[0]))
                    }).unwrap().await;
                } else if handle == server.mansion_lighting.skip {
                    info!("setting skip");
                    server.get(server.mansion_lighting.skip, |value| {
                        sender.send(Message::SetSkip(value[0]))
                    }).unwrap().await;
                } else {
                    info!("[gatt] Write event on {:?}", handle);
                }
            }
            Ok(GattEvent::Read { handle, connection: _ }) => {
                info!("[gatt] Read event on {:?}", handle);
            }
            Err(e) => {
                error!("[gatt] Error processing GATT events: {:?}", e);
            }
        }
    }
}

async fn advertise_task<C: Controller>(
    mut peripheral: Peripheral<'_, C>,
    server: &Server<'_, '_, C>,
) -> Result<(), BleHostError<C::Error>> {
    let mut adv_data = [0; 31];
    AdStructure::encode_slice(
        &[
            AdStructure::Flags(LE_GENERAL_DISCOVERABLE | BR_EDR_NOT_SUPPORTED),
            AdStructure::ServiceUuids16(&[Uuid::Uuid16([0x0f, 0x18])]),
            AdStructure::CompleteLocalName(b"mansion lighting"),
        ],
        &mut adv_data[..],
    )?;
    loop {
        info!("[adv] advertising");
        let mut advertiser = match peripheral
            .advertise(
                &Default::default(),
                Advertisement::ConnectableScannableUndirected {
                    adv_data: &adv_data[..],
                    scan_data: &[],
                },
            )
            .await {
                Ok(x) => x,
                Err(e) => {
                    error!("ADVERTISING ERROR: {:?}", e);
                    return Err(e);
                }
        };
        info!("[adv] advertising2");
        let conn = advertiser.accept().await?;
        info!("[adv] connection established");
        // wait until connection dies
        while conn.is_connected() {
            yield_now().await;
        }
    }
}
