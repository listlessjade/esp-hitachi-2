#![feature(c_variadic)]

use std::{
    ffi::{CStr, CString},
    fs::File,
    sync::Arc,
};

use ble::LovenseMessage;
use conf::{Config, MotorConfig, RemoteLogConfig, WifiConfig};
use conn::{ble, http::run_http, remote_log::remote_log_server, serial::SerialHandler};
use esp_idf_hal::{
    ledc::{config::TimerConfig, LedcDriver, LedcTimerDriver},
    prelude::Peripherals,
    temp_sensor::{TempSensorConfig, TempSensorDriver},
    units::KiloHertz,
};
use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    nvs::EspDefaultNvsPartition,
    ota::EspOta,
    wifi::EspWifi,
};
use esp_idf_sys::{
    esp_app_get_description, esp_nofail, esp_vfs_littlefs_conf_t, esp_vfs_littlefs_register,
    EspError,
};
use idf_libs::{
    button::{ButtonConfig, ButtonEvent, ButtonManager},
    log_redirection::{log_crate_shenanigans::EspChannelLogger, redirect_logs},
    ntc::{Thermistor, ThermistorConfig},
};
use lights::Lights;
use motor::Motor;
use parking_lot::Mutex;
use thingbuf::recycling::WithCapacity;
pub mod conf;
pub mod conn;
pub mod event_queue;
pub mod idf_libs;
pub mod lights;
pub mod motor;
pub mod wifi;

pub type EspResult<T> = Result<T, EspError>;

#[no_mangle]
extern "C" fn rust_primary() -> i32 {
    // It is necessary to call this function once. Otherwise some patches to the runtime
    // implemented by esp-idf-sys might not link properly. See https://github.com/esp-rs/esp-idf-template/issues/71
    esp_idf_svc::sys::link_patches();

    // Bind the log crate to the ESP Logging facilities
    // esp_idf_svc::log::EspLogger::initialize_default();

    log::info!("Hello, world!");

    if let Err(e) = real_main() {
        log::error!("Main errored out: {e}");
    }

    42
}

fn real_main() -> anyhow::Result<()> {
    let peripherals = Peripherals::take()?;
    let sys_loop = EspSystemEventLoop::take()?;
    let nvs = EspDefaultNvsPartition::take()?;

    let led_pin = peripherals.pins.gpio10;
    let mut lights = Lights::new(led_pin)?;

    lights.set_all([(0, 0, 100), (0, 0, 0), (0, 0, 0), (0, 0, 0)])?;

    if let Some(desc) = unsafe { esp_app_get_description().as_ref() } {
        println!("hewwo from the hitachi!");
        println!(
            "built on -- time {} date {}",
            CStr::from_bytes_until_nul(&desc.time)
                .unwrap()
                .to_string_lossy(),
            CStr::from_bytes_until_nul(&desc.date)
                .unwrap()
                .to_string_lossy()
        );
    }

    unsafe {
        let base_path = CString::new("/littlefs").unwrap();
        let storage = CString::new("storage").unwrap();

        let mut conf = esp_vfs_littlefs_conf_t {
            base_path: base_path.as_ptr(),
            partition_label: storage.as_ptr(),
            ..Default::default()
        };

        conf.set_format_if_mount_failed(1);
        conf.set_read_only(0);
        conf.set_grow_on_mount(1);

        esp_nofail!(esp_vfs_littlefs_register(&conf));
    }

    if !std::fs::exists("/littlefs/config.json")? {
        serde_json::to_writer(
            File::create("/littlefs/config.json")?,
            &Config {
                wifi: WifiConfig {
                    enable: true,
                    ssid: String::new(),
                    password: String::new(),
                    auth: conf::WifiAuthMethod::Personal,
                    identity: String::new(),
                    username: String::new(),
                },
                motor: MotorConfig {
                    max_power: 100,
                    min_power: 50,
                },
                remote_log: RemoteLogConfig {
                    enable: true,
                    port: 8070,
                },
            },
        )?;
    }

    let wifi_cfg: WifiConfig =
        serde_json::from_reader::<_, Config>(File::open("/littlefs/config.json")?)?.wifi;
    let mut wifi_manager = wifi::WifiManager::new(
        EspWifi::new(peripherals.modem, sys_loop.clone(), Some(nvs))?,
        sys_loop,
    )?;
    if let Err(e) = wifi_manager.start() {
        log::error!("failed to start up wifi: {e}");
    }

    lights.set_all([(0, 0, 0), (0, 0, 0), (0, 0, 0), (100, 100, 100)])?;

    // info!("Wifi DHCP info: {:?}", wifi_manager.);

    let (log_tx, log_rx) = redirect_logs();
    EspChannelLogger::initialize_with_channel(log_tx);

    let log_thread = std::thread::spawn(move || remote_log_server("0.0.0.0:8070", log_rx));

    let timer_driver = LedcTimerDriver::new(
        peripherals.ledc.timer0,
        &TimerConfig::default().frequency(KiloHertz(5).into()),
    )?;
    let driver = LedcDriver::new(
        peripherals.ledc.channel0,
        timer_driver,
        peripherals.pins.gpio5,
    )?;

    let mut motor = Motor::new(driver);

    let thermistor = Thermistor::new(
        peripherals.pins.gpio2,
        ThermistorConfig {
            b_value: 3950,
            r25_ohm: 10000,
            fixed_ohm: 10000,
            vdd_mv: 3300,
            circuit_mode: idf_libs::ntc::ThermistorCircuitMode::NtcGnd,
        },
    )?;

    let mut temp_sensor = TempSensorDriver::new(&TempSensorConfig::new(), peripherals.temp_sensor)?;
    temp_sensor.enable()?;
    let serial_handler = SerialHandler::new(thermistor, temp_sensor);

    // driver.set_duty(max_duty * 3 / 4)?;

    let (event_tx, event_rx) = event_queue::get_channel();

    let mut button_manager = ButtonManager::new(event_tx.clone());
    button_manager.add_button(peripherals.pins.gpio6, ButtonConfig::default())?;
    button_manager.add_button(peripherals.pins.gpio7, ButtonConfig::default())?;
    button_manager.add_button(peripherals.pins.gpio8, ButtonConfig::default())?;

    let ota = Arc::new(Mutex::new(EspOta::new().unwrap()));

    let ble_tx = event_tx.clone();
    let (uart_tx_send, uart_tx_receive) =
        thingbuf::mpsc::blocking::with_recycle(32, WithCapacity::new().with_max_capacity(128));
    let (uart_rx_send, uart_rx_receive) =
        thingbuf::mpsc::blocking::with_recycle(32, WithCapacity::new().with_max_capacity(128));

    std::thread::spawn(|| ble::run_ble(ble_tx, uart_tx_receive, uart_rx_send));
    let http = run_http(8080, Arc::clone(&ota))?;

    std::thread::spawn(move || serial_handler.handle_serial(uart_rx_receive, uart_tx_send));

    for event in &event_rx {
        match *event {
            event_queue::Event::Button(ButtonEvent::SingleClick, pin) => {
                let speed = match pin {
                    6 => motor.inc(),
                    7 => motor.dec(),
                    8 => motor.set(0),
                    _ => continue,
                };

                lights.show_speed(speed)?;
            }
            event_queue::Event::SetPwm(val) => {
                // let val = val as u32;

                // let
                // driver.set_duty(driver.get_max_duty() * val / (u8::MAX as u32))?;
            }
            event_queue::Event::Lovense(LovenseMessage::Vibrate(val)) => {
                lights.show_speed(motor.set(val as u32))?;
            }
            _ => continue,
        }
    }

    Ok(())
}


// rust analyzer gets angry if this is missing /shrug
fn main() {}
