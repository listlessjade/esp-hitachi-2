use esp_idf_hal::{gpio::ADCPin, temp_sensor::TempSensorDriver};
use parking_lot::Mutex;
use thingbuf::{
    mpsc::blocking::{Receiver, Sender},
    recycling::WithCapacity,
};

use esp_idf_sys::{esp_get_free_heap_size, esp_get_minimum_free_heap_size};
use getargs::{Opt, Options};
use humansize::DECIMAL;
use std::{
    fs::File,
    io::{Seek, Write},
    ops::DerefMut,
    sync::Arc,
};
use thingbuf::mpsc::blocking::SendRef;

use crate::{conf::Config, idf_libs::ntc::Thermistor};

static HELP: &str = "USAGE: 
wifi --field [FIELD] get|set [VALUE] | set wifi config options
restart | self-explanatory
dump-config
sys mem|temp|[bweh]
help
";
static WIFI_HELP: &str = "USAGE:
wifi --field [FIELD] get
wifi --field [FIELD] set [VALUE]
";

pub struct SerialHandler<P: ADCPin> {
    ntc: Arc<Mutex<Thermistor<P>>>,
    temp_sensor: TempSensorDriver<'static>,
    // timer_service: EspTimerService<Task>,
    // timer: Option<EspTimer<'static>>
}

impl<P: ADCPin> SerialHandler<P> {
    pub fn new(ntc: Thermistor<P>, temp_sensor: TempSensorDriver<'static>) -> Self {
        Self {
            ntc: Arc::new(Mutex::new(ntc)),
            temp_sensor,
            // timer_service: EspTimerService::new().unwrap(),
            // timer: None
        }
    }

    pub fn handle_cmd(
        &mut self,
        recv: &str,
        output: &mut SendRef<'_, Vec<u8>>,
    ) -> anyhow::Result<()> {
        let args = shlex::split(recv).ok_or_else(|| anyhow::anyhow!("invalid string"))?;
        let mut parser = Options::new(args.iter().map(String::as_str));
        while let Some(opt) = parser.next_opt().ok().flatten() {
            match opt {
                Opt::Short('h') | Opt::Long("help") => {
                    writeln!(output, "{HELP}")?;
                }
                _ => {}
            }
        }

        let mut config_f = File::options()
            .write(true)
            .read(true)
            .open("/littlefs/config.json")?;
        let mut config: Config = serde_json::from_reader(&mut config_f)?;

        let res = match parser.next_positional() {
            Some("wifi") => self.handle_wifi(&mut parser, &mut config, output),
            Some("restart") => esp_idf_hal::reset::restart(),
            Some("dump-config") => {
                write!(output, "Current configuration: ")?;
                serde_json::to_writer_pretty(output.deref_mut(), &config)?;
                writeln!(output.deref_mut())?;
                Ok(())
            }
            Some("sys") => self.handle_sys(&mut parser, &mut config, output),
            // Some("monitor") => {
            //     self.handle_monitor(&mut parser, &mut config, output)
            // }
            Some("help") => {
                writeln!(output, "{HELP}")?;
                Ok(())
            }
            _ => {
                writeln!(output, "Invalid subcommand! Usage: {HELP}")?;
                return Ok(());
            }
        };

        if let Err(e) = res {
            writeln!(output, "Error!: {e}");
            return Ok(());
        }

        config_f.rewind()?;
        config_f.set_len(0)?;
        serde_json::to_writer(config_f, &config)?;

        Ok(())
    }

    pub fn handle_sys<'args, I: Iterator<Item = &'args str>>(
        &mut self,
        parser: &mut Options<&'args str, I>,
        config: &mut Config,
        output: &mut SendRef<'_, Vec<u8>>,
    ) -> anyhow::Result<()> {
        while let Some(opt) = parser.next_opt().ok().flatten() {}

        let Some(field) = parser.next_positional() else {
            return Err(anyhow::anyhow!(
                "Missing field to get value for - Usage: sys mem|temp"
            ));
        };

        match field {
            "mem" => writeln!(
                output,
                "Free Memory: {} (hist. minimum {})",
                humansize::format_size(unsafe { esp_get_free_heap_size() }, DECIMAL),
                humansize::format_size(unsafe { esp_get_minimum_free_heap_size() }, DECIMAL)
            )?,
            "temp" => writeln!(output, "Motor temp: {:.2}°C.", self.ntc.lock().get_temp()?)?,
            _ => {
                return Err(anyhow::anyhow!(
                    "Invalid field {field}. Valid fields are mem and temp"
                ))
            }
        }

        Ok(())
    }

    // pub fn handle_monitor<'args, I: Iterator<Item = &'args str>>(
    //     &mut self,
    //     parser: &mut Options<&'args str, I>,
    //     config: &mut Config,
    //     output: &mut SendRef<'_, Vec<u8>>,
    // ) -> anyhow::Result<()> {
    //     while let Some(opt) = parser.next_opt().ok().flatten() {};

    //     let Some(field) = parser.next_positional() else {
    //         return Err(anyhow::anyhow!("Missing field to get value for - Usage: sys mem|temp"))
    //     };

    //     match field {
    //         "mem" => {
    //             let timer = self.timer_service.timer(||)
    //         }
    //         writeln!(output, "Free Memory: {} (hist. minimum {})", humansize::format_size(unsafe { esp_get_free_heap_size() }, DECIMAL), humansize::format_size(unsafe { esp_get_minimum_free_heap_size() }, DECIMAL))?,
    //         "temp" => writeln!(output, "Motor temp: {:.2}°C.", self.ntc.lock().get_temp()?)?,
    //         "stop" => {
    //             let Some(timer) = self.timer.take() else {
    //                 return Ok(());
    //             };

    //             drop(timer);
    //             Ok(())
    //         }
    //         _ => return Err(anyhow::anyhow!("Invalid field {field}. Valid fields are mem and temp"))

    //     }
    // }

    pub fn handle_wifi<'args, I: Iterator<Item = &'args str>>(
        &mut self,
        parser: &mut Options<&'args str, I>,
        config: &mut Config,
        output: &mut SendRef<'_, Vec<u8>>,
    ) -> anyhow::Result<()> {
        let mut field = None;

        while let Some(opt) = parser.next_opt().ok().flatten() {
            match opt {
                Opt::Short('f') | Opt::Long("field") => {
                    field = Some(
                        parser
                            .value()
                            .map_err(|_| anyhow::anyhow!("couldn't parse option value"))?,
                    )
                }
                _ => {}
            }
        }

        match parser.next_positional() {
            Some("set") => {
                let Some(field) = field else {
                    return Err(anyhow::anyhow!("missing --field. usage: {WIFI_HELP}"));
                };

                let Some(value) = parser.next_positional() else {
                    return Err(anyhow::anyhow!(
                        "missing value to set field to. usage: {WIFI_HELP}"
                    ));
                };

                config.wifi.update(field, value)?;

                writeln!(output, "Updated {field} to {value}!")?;
            }
            Some("get") => {
                let Some(field) = field else {
                    return Err(anyhow::anyhow!("missing --field. usage: {WIFI_HELP}"));
                };

                writeln!(
                    output,
                    "Field {field} is set to {}",
                    config.wifi.get(field)?
                )?;
            }
            _ => return Err(anyhow::anyhow!("Invalid subcommand")),
        }

        Ok(())
    }

    pub fn handle_serial(
        mut self,
        uart_rx: Receiver<String, WithCapacity>,
        uart_tx: Sender<Vec<u8>, WithCapacity>,
    ) {
        while let Some(req_slot) = uart_rx.recv_ref() {
            log::info!("Received on ble/UART: {req_slot}");
            let mut send_slot = uart_tx.send_ref().unwrap();

            if let Err(e) = self.handle_cmd(req_slot.as_str(), &mut send_slot) {
                let _ = writeln!(send_slot, "Error while handling: {e}");
            }
        }
    }
}
