const LOVENSE_RX_CHAR: BleUuid = uuid128!("54300002-0023-4bd4-bbd5-a6920e4c5653");
const LOVENSE_TX_CHAR: BleUuid = uuid128!("54300003-0023-4bd4-bbd5-a6920e4c5653");

const LOVENSE_SERVICE_ID: BleUuid = uuid128!("54300001-0023-4bd4-bbd5-a6920e4c5653");

const NUS_SERVICE_ID: BleUuid = uuid128!("6E400001-B5A3-F393-E0A9-E50E24DCCA9E");
const NUS_RX_CHAR: BleUuid = uuid128!("6E400002-B5A3-F393-E0A9-E50E24DCCA9E");
const NUS_TX_CHAR: BleUuid = uuid128!("6E400003-B5A3-F393-E0A9-E50E24DCCA9E");

use std::{sync::Arc, time::Duration};

use esp32_nimble::{
    enums::{AuthReq, SecurityIOCap},
    utilities::BleUuid,
    uuid128, BLEAdvertisementData, BLEDevice, NimbleProperties,
};
use thingbuf::{
    mpsc::blocking::{Receiver, Sender, StaticSender},
    recycling::WithCapacity,
};

use crate::event_queue::Event;

pub fn run_ble(
    sender: StaticSender<Event>,
    uart_tx: Receiver<Vec<u8>, WithCapacity>,
    uart_rx: Sender<String, WithCapacity>,
) {
    let device = BLEDevice::take();

    // device
    device
        .security()
        .set_auth(AuthReq::Bond) // Bonding enables key storage for reconnection
        .set_passkey(123456) // Optional, sets the passkey for pairing
        .set_io_cap(SecurityIOCap::NoInputNoOutput) // You can choose any IO capability
        .resolve_rpa(); // Crucial for managing iOS's dynamic Bluetooth addresses

    let advertising = device.get_advertising();

    let server = device.get_server();

    server.on_connect(|server, desc| {
        log::info!("hewwo to {desc:?}");
        if server.connected_count() < (esp_idf_svc::sys::CONFIG_BT_NIMBLE_MAX_CONNECTIONS as _) {
            log::info!("Multi-connect support: start advertising");
            advertising.lock().start().unwrap();
        }
    });

    server.on_disconnect(|desc, reason| {
        log::info!("{desc:?} has left: {reason:?}");
    });

    server.on_authentication_complete(|desc, result| {
        log::info!("auth completed: {desc:?}: {result:?}")
    });

    let lovense_service = server.create_service(LOVENSE_SERVICE_ID);

    let lovense_rx = lovense_service.lock().create_characteristic(
        LOVENSE_RX_CHAR,
        NimbleProperties::WRITE | NimbleProperties::WRITE_NO_RSP,
    );

    let lovense_tx = lovense_service.lock().create_characteristic(
        LOVENSE_TX_CHAR,
        NimbleProperties::READ | NimbleProperties::NOTIFY,
    );

    let lovense_sender = sender.clone();

    lovense_rx.lock().on_write(move |args| {
        if let Some(msg) = std::str::from_utf8(args.recv_data())
            .ok()
            .and_then(LovenseMessage::parse)
        {
            lovense_sender.try_send(Event::Lovense(msg));
        }
        // let _ = lovense.req_tx.send(args.recv_data().to_vec());
        // println!("from lovense: {}", std::str::from_utf8(args.recv_data()).unwrap());
    });

    let nus_service = server.create_service(NUS_SERVICE_ID);
    let nus_rx = nus_service.lock().create_characteristic(
        NUS_RX_CHAR,
        NimbleProperties::WRITE | NimbleProperties::WRITE_NO_RSP,
    );

    let nus_tx = nus_service.lock().create_characteristic(
        NUS_TX_CHAR,
        NimbleProperties::READ | NimbleProperties::NOTIFY,
    );

    let nus_tx_handle = Arc::clone(&nus_tx);

    nus_rx.lock().on_write(move |args| {
        let Some(msg) = std::str::from_utf8(args.recv_data()).ok() else {
            return;
        };
        if let Some(msg) = LovenseMessage::parse(msg) {
            sender.try_send(Event::Lovense(msg));
        } else if let Ok(mut res_slot) = uart_rx.try_send_ref() {
            res_slot.clear();
            res_slot.push_str(msg);
        }
        // if let Ok(data) = std::str::from_utf8(args.recv_data()) {
        //     log::info!("NUS RX: {data}");
        //     nus_tx_handle.lock().set_value(data.as_bytes());
        //     nus_tx_handle.lock().notify();
        // }
    });

    advertising
        .lock()
        .set_data(
            BLEAdvertisementData::new()
                .name("LOVE-Calor")
                .add_service_uuid(LOVENSE_SERVICE_ID), // .add_service_uuid(ESPWAND_SERVICE_ID)
                                                       // .add_service_uuid(LOVENSE_SERVICE_ID),
        )
        .unwrap();

    advertising.lock().start().unwrap();

    while let Some(res_slot) = uart_tx.recv_ref() {
        let mut handle = nus_tx.lock();
        handle.set_value(&res_slot);
        handle.notify();
    }

    loop {
        std::thread::sleep(Duration::from_millis(1000));
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LovenseMessage {
    Vibrate(u8),
    Unrecognized,
}

impl LovenseMessage {
    pub fn parse(args: &str) -> Option<LovenseMessage> {
        let Some(msg_end) = args.find(';') else {
            return None;
        };

        let args: Vec<&str> = args[..msg_end].split_terminator(':').collect();

        match args[0] {
            "Vibrate" => args
                .get(1)
                .and_then(|v| v.parse::<u8>().ok())
                .map(LovenseMessage::Vibrate),
            _ => Some(LovenseMessage::Unrecognized),
        }
    }
}
