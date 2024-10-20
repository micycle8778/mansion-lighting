#![no_std]
#![no_main]
#![deny(unused_must_use)]

use core::fmt::Write;

use trouble_host::attribute::Uuid;
use trouble_host::advertise::BR_EDR_NOT_SUPPORTED;
use trouble_host::advertise::LE_GENERAL_DISCOVERABLE;
use trouble_host::advertise::AdStructure;
use trouble_host::attribute::CharacteristicProp;
use embassy_time::Duration;
use embassy_time::Timer;
use trouble_host::advertise::Advertisement;
use trouble_host::gatt::GattEvent;
use embassy_futures::join::join3;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use trouble_host::attribute::Service;
use trouble_host::attribute::AttributeTable;
use trouble_host::Address;
use trouble_host::BleHost;
use trouble_host::BleHostResources;
use trouble_host::PacketQos;
use bt_hci::controller::ExternalController;
use static_cell::StaticCell;

use cyw43_pio::PioSpi;

use embassy_executor::Spawner;
use embassy_rp::gpio::Output;
use embassy_rp::gpio::Level;
use embassy_rp::i2c::{self, I2c};
use embassy_rp::pio::Pio;

use embassy_rp::bind_interrupts;
use embassy_rp::peripherals::DMA_CH0;
use embassy_rp::peripherals::I2C0;
use embassy_rp::peripherals::PIO0;
use embassy_rp::pio::InterruptHandler as PIOInterruptHandler;
use embassy_rp::i2c::InterruptHandler as I2CInterruptHandler;

use defmt as _;
use defmt_rtt as _;

use ssd1306::{prelude::*, Ssd1306};
use ssd1306::I2CDisplayInterface;

use emb_test::led::{Color, LedDriver};
// Bind interrupts to their handlers.
bind_interrupts!(struct Irqs {
    I2C0_IRQ => I2CInterruptHandler<I2C0>;
    PIO0_IRQ_0 => PIOInterruptHandler<PIO0>;
});

#[embassy_executor::task]
async fn cyw43_task(runner: cyw43::Runner<'static, Output<'static>, PioSpi<'static, PIO0, 1, DMA_CH0>>) -> ! {
    runner.run().await
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    // Initialize peripherals and USB driver.
    let p = embassy_rp::init(Default::default());

    // initialize the OLED (SSD1306)
    let i2c = I2c::new_async(
        p.I2C0,
        p.PIN_1,
        p.PIN_0, 
        Irqs,
        i2c::Config::default()
    );
    let interface = I2CDisplayInterface::new(i2c);
    let mut display = Ssd1306::new(
        interface,
        DisplaySize128x64,
        DisplayRotation::Rotate0,
    ).into_terminal_mode();
    display.init().unwrap();
    display.clear().unwrap();

    // setup PIO
    let mut pio = Pio::new(p.PIO0, Irqs); 

    // initialize the w2812 LEDs
    let mut leds = {
        LedDriver::new(&mut pio.common, pio.sm0, p.PIN_28)
    };

    // initialize the bluetooth chip
    // first, lets get the firmware in here. we need this firmware to use
    // the onboard bluetooth chip
    let fw = include_bytes!("../firmware/43439A0.bin");
    let clm = include_bytes!("../firmware/43439A0_clm.bin");
    let btfw = include_bytes!("../firmware/43439A0_btfw.bin");

    // setup pins for talking to the SPI bus of the bluetooth chip
    let pwr = Output::new(p.PIN_23, Level::Low);
    let cs = Output::new(p.PIN_25, Level::High);
    let spi = PioSpi::new(&mut pio.common, pio.sm1, pio.irq0, cs, p.PIN_24, p.PIN_29, p.DMA_CH0);

    // spin up the driver
    static STATE: StaticCell<cyw43::State> = StaticCell::new();
    let state = STATE.init(cyw43::State::new());
    let (_net_device, bt_device, mut control, runner) = cyw43::new_with_bluetooth(state, pwr, spi, fw, btfw).await;
    spawner.spawn(cyw43_task(runner)).unwrap();
    control.init(clm).await;

    let controller: ExternalController<_, 10> = ExternalController::new(bt_device);
    static HOST_RESOURCES: StaticCell<BleHostResources<4, 32, 27>> = StaticCell::new();
    let host_resources = HOST_RESOURCES.init(BleHostResources::new(PacketQos::None));

    let mut ble = BleHost::new(controller, host_resources);

    ble.set_random_address(Address::random([0xff, 0x9f, 0x1a, 0x05, 0xe4, 0xff]));
    let mut table: AttributeTable<'_, NoopRawMutex, 32> = AttributeTable::new();

    // Generic Access Service (mandatory)
    let id = b"Pico W Bluetooth";
    let appearance = [0x80, 0x07];
    let mut bat_level = [0; 1];
    let mut my_byte = [0; 4];
    let handle = {
        table.add_service(Service::new(0x1801)); // Generic attribute service (mandatory)
        let mut svc = table.add_service(Service::new(0x1800));
        let _ = svc.add_characteristic_ro(0x2a00, id);
        let _ = svc.add_characteristic_ro(0x2a01, &appearance[..]);
        svc.build();

        // random new service
        table.add_service(Service::new(0x6969))
            .add_characteristic(0x6969, &[CharacteristicProp::Read, CharacteristicProp::Write], &mut my_byte);

        // Battery service
        let mut svc = table.add_service(Service::new(0x180f));

        svc.add_characteristic(
            0x2a19,
            &[CharacteristicProp::Read, CharacteristicProp::Notify],
            &mut bat_level,
        )
        .build()
    };

    let mut adv_data = [0; 31];
    AdStructure::encode_slice(
        &[
            AdStructure::Flags(LE_GENERAL_DISCOVERABLE | BR_EDR_NOT_SUPPORTED),
            AdStructure::ServiceUuids16(&[Uuid::Uuid16([0x0f, 0x18])]),
            AdStructure::CompleteLocalName(b"Pico W Bluetooth"),
        ],
        &mut adv_data[..],
    )
    .unwrap();

    let server = ble.gatt_server(&table);

    let _ = join3(
        ble.run(),
        async {
            loop {
                match server.next().await {
                    Ok(GattEvent::Write { handle, connection: _ }) => {
                        table.get(handle, |value| {
                            display.clear().unwrap();
                            write!(display, "handle: {handle:?}; value: {value:?}").unwrap();
                            // info!("Write event. Value written: {:?}", value);
                        }).unwrap();
                    }
                    Ok(GattEvent::Read { .. }) => {
                        // let _ = display.clear();
                        // let _= write!(display, "Read event");
                        // info!("Read event");
                    }
                    Err(e) => {
                        let _ = display.clear();
                        let _ = write!(display, "Error processing GATT events: {:?}", e);
                        // error!("Error processing GATT events: {:?}", e);
                    }
                }
            }
        },
        async {
            let mut advertiser = ble
                .advertise(
                    &Default::default(),
                    Advertisement::ConnectableScannableUndirected {
                        adv_data: &adv_data[..],
                        scan_data: &[],
                    },
                )
                .await
                .unwrap();
            let conn = advertiser.accept().await.unwrap();
            // Keep connection alive
            let mut tick: u8 = 0;
            loop {
                Timer::after(Duration::from_secs(10)).await;
                tick += 1;
                server.notify(handle, &conn, &[tick]).await.unwrap();
            }
        },
        )
            .await;

    panic!("end of program.");
}
