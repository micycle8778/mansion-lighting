use embassy_futures::yield_now;
use embassy_sync::blocking_mutex::raw::RawMutex;
use embassy_sync::channel::Sender;
use log::error;
use log::info;

use embassy_futures::select::select3;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use trouble_host::prelude::*;

use crate::lighting::Message;
use crate::Color;

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
struct Server {}

struct Handles {
    base_color: Characteristic,
    brightness: Characteristic,
    skip: Characteristic,
    speed: Characteristic,
    animation: Characteristic,
}

const fn gen_uuid(s: &str) -> Uuid {
    let bytes = s.as_bytes();
    assert!(bytes.len() <= 16);
    let mut result = [0u8; 16];

    let mut idx = 0;
    while idx != s.len() {
        result[result.len() - 1 - idx] = bytes[idx];

        idx += 1;
    }

    Uuid::new_long(result)
}

pub async fn run<C: Controller, M: RawMutex, const N: usize>(
    controller: C,
    sender: Sender<'_, M, Message, N>,
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

    // mansion lighting
    // we're avoiding the host_macro stuff because those use static_cell
    // which panic if they're used more than once
    let mut base_color = [0u8; 3];
    let mut brightness = [0u8];
    let mut skip = [0u8];
    let mut animation_speed = [0u8; 4];
    let mut animation = [0u8; 16];

    let handles = {
        const SERVICE_UUID: Uuid = gen_uuid("michaels mansion");
        const BASE_COLOR_UUID: Uuid = gen_uuid("base color");
        const BRIGHTNESS_UUID: Uuid = gen_uuid("brightness");
        const SKIP_UUID: Uuid = gen_uuid("skip");
        const ANIMATION_UUID: Uuid = gen_uuid("animation");
        const SPEED_UUID: Uuid = gen_uuid("speed");

        let mut service = table.add_service(Service::new(SERVICE_UUID));

        let base_color = service
            .add_characteristic(
                BASE_COLOR_UUID,
                &[CharacteristicProp::Write],
                &mut base_color,
            )
            .build();

        let brightness = service
            .add_characteristic(
                BRIGHTNESS_UUID,
                &[CharacteristicProp::Write],
                &mut brightness,
            )
            .build();

        let skip = service
            .add_characteristic(SKIP_UUID, &[CharacteristicProp::Write], &mut skip)
            .build();

        let speed = service
            .add_characteristic(
                SPEED_UUID,
                &[CharacteristicProp::Write],
                &mut animation_speed,
            )
            .build();

        let animation = service
            .add_characteristic(ANIMATION_UUID, &[CharacteristicProp::Write], &mut animation)
            .build();

        service.build();

        Handles {
            base_color,
            brightness,
            skip,
            speed,
            animation,
        }
    };

    let server = Server::new(stack, &mut table);

    info!("Starting advertising and GATT service");
    let _ = select3(
        ble_task(runner),
        gatt_task(&server, sender, handles),
        advertise_task(peripheral),
    )
    .await;
}

async fn ble_task<C: Controller>(mut runner: Runner<'_, C>) {
    if let Err(e) = runner.run().await {
        error!("ble_task ERROR: {e:?}");
    }
}

async fn gatt_task<C: Controller, M: RawMutex, const N: usize>(
    server: &Server<'_, '_, C>,
    sender: Sender<'_, M, Message, N>,
    handles: Handles,
) {
    loop {
        match server.next().await {
            Ok(GattEvent::Write {
                handle,
                connection: _,
            }) => {
                info!("[gatt] pre write event on {:?}", handle);

                if handle == handles.base_color {
                    info!("setting base color");
                    server
                        .get(handles.base_color, |value| {
                            let color = Color::new(value[0], value[1], value[2]);
                            sender.send(Message::SetColor(color))
                        })
                        .unwrap()
                        .await;
                } else if handle == handles.brightness {
                    info!("setting brightness");
                    server
                        .get(handles.brightness, |value| {
                            sender.send(Message::SetBrightness(value[0]))
                        })
                        .unwrap()
                        .await;
                } else if handle == handles.skip {
                    info!("setting skip");
                    server
                        .get(handles.skip, |value| {
                            sender.send(Message::SetSkip(value[0]))
                        })
                        .unwrap()
                        .await;
                } else if handle == handles.speed {
                    server
                        .get(handles.speed, |value| {
                            let Ok(bytes) = value.try_into() else {
                                return sender.send(Message::Noop);
                            };
                            let speed = f32::from_le_bytes(bytes);
                            if speed.is_finite() {
                                sender.send(Message::SetAnimationSpeed(speed))
                            } else {
                                sender.send(Message::Noop)
                            }
                        })
                        .unwrap()
                        .await;
                } else if handle == handles.animation {
                    server
                        .get(handles.animation, |value| {
                            sender.send(Message::UseAnimation(value.try_into().unwrap()))
                        })
                        .unwrap()
                        .await;
                } else {
                    info!("[gatt] Write event on {:?}", handle);
                }
            }
            Ok(GattEvent::Read {
                handle,
                connection: _,
            }) => {
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
            .await
        {
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
