use std::time::Duration;

use esp_idf_svc::{
    eventloop::{EspEventLoop, System},
    wifi::{AuthMethod, BlockingWifi, EspWifi},
};
use esp_idf_sys::{
    esp, esp_eap_client_set_identity, esp_eap_client_set_password, esp_eap_client_set_username,
    esp_wifi_sta_enterprise_enable,
};

use crate::conf::{self, WifiConfig};

pub struct WifiManager {
    wifi: BlockingWifi<EspWifi<'static>>,
}

impl WifiManager {
    pub fn new(wifi: EspWifi<'static>, eloop: EspEventLoop<System>) -> anyhow::Result<Self> {
        Ok(WifiManager {
            wifi: BlockingWifi::wrap(wifi, eloop)?,
        })
    }

    pub fn set_config(&mut self, config: &WifiConfig) -> anyhow::Result<()> {
        let mut esp_config_base = self.wifi.get_configuration()?;
        let esp_config = esp_config_base.as_client_conf_mut();

        esp_config.ssid.clear();
        esp_config.ssid.push_str(&config.ssid);

        match config.auth {
            conf::WifiAuthMethod::Personal => {
                esp_config.auth_method = AuthMethod::WPA2Personal;
                esp_config.password.clear();
                esp_config.password.push_str(&config.password);
            }
            conf::WifiAuthMethod::Enterprise => {
                esp_config.auth_method = AuthMethod::WPA2Enterprise;

                esp!(unsafe {
                    esp_eap_client_set_identity(
                        config.identity.as_ptr(),
                        config.identity.len() as i32,
                    )
                })?;

                esp!(unsafe {
                    esp_eap_client_set_username(
                        config.username.as_ptr(),
                        config.username.len() as i32,
                    )
                })?;

                esp!(unsafe {
                    esp_eap_client_set_password(
                        config.password.as_ptr(),
                        config.password.len() as i32,
                    )
                })?;

                esp!(unsafe { esp_wifi_sta_enterprise_enable() })?;
            }
        };

        self.wifi.set_configuration(&esp_config_base)?;

        Ok(())
    }

    pub fn start(&mut self) -> anyhow::Result<()> {
        const WIFI_TIMEOUT: Duration = Duration::from_secs(5);

        self.wifi.start()?;
        log::info!("Wifi started");

        self.wifi.wifi_mut().connect()?;
        self.wifi
            .wifi_wait_while(|| self.wifi.is_connected().map(|s| !s), Some(WIFI_TIMEOUT))?;
        log::info!("Wifi connected!");

        self.wifi.wait_netif_up()?;
        log::info!("Wifi/Netif up!");

        log::info!("IP: {:?}", self.wifi.wifi().sta_netif().get_ip_info()?);

        Ok(())
    }

    pub fn stop(&mut self) -> anyhow::Result<()> {
        self.wifi.disconnect()?;
        log::info!("wifi disconnected");
        self.wifi.stop()?;
        log::info!("wifi stopped");

        Ok(())
    }
}
