use std::fmt::Display;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub wifi: WifiConfig,
    pub motor: MotorConfig,
    pub remote_log: RemoteLogConfig,
}

#[derive(Serialize, Deserialize)]
pub struct WifiConfig {
    pub enable: bool,
    pub ssid: String,
    pub password: String,
    pub auth: WifiAuthMethod,
    pub identity: String,
    pub username: String,
}

impl WifiConfig {
    pub fn update(&mut self, field: &str, val: &str) -> anyhow::Result<()> {
        match field {
            "enable" => {
                self.enable = if ["yes", "y", "true"].contains(&val) {
                    true
                } else if ["no", "n", "false"].contains(&val) {
                    false
                } else {
                    return Err(anyhow::anyhow!("Invalid value for 'enabled': {val}. Valid values are true/yes and false/no"));
                };
            }
            "ssid" => {
                self.ssid = val.into();
            }
            "password" => {
                self.password = val.into();
            }
            "auth" => {
                self.auth = if ["WPA2_PERSONAL", "Personal", "personal"].contains(&val) {
                    WifiAuthMethod::Personal
                } else if ["WPA2_ENTERPRISE", "Enterprise", "enterprise"].contains(&val) {
                    WifiAuthMethod::Enterprise
                } else {
                    return Err(anyhow::anyhow!("Invalid value for 'auth': {val}. Valid values are WPA2_PERSONAL and WPA2_ENTERPRISE"));
                }
            }
            "identity" => {
                self.identity = val.into();
            }
            "username" => {
                self.username = val.into();
            }
            _ => return Err(anyhow::anyhow!("Invalid field {field}")),
        }

        Ok(())
    }

    pub fn get(&mut self, field: &str) -> anyhow::Result<String> {
        Ok(match field {
            "enable" => self.enable.to_string(),
            "ssid" => self.ssid.clone(),
            "password" => self.password.clone(),
            "auth" => self.auth.to_string(),
            "identity" => self.identity.to_string(),
            "username" => self.username.to_string(),
            _ => return Err(anyhow::anyhow!("Invalid field {field}")),
        })
    }
}

#[derive(Serialize, Deserialize)]
pub struct MotorConfig {
    pub max_power: i64,
    pub min_power: i64,
}

#[derive(Serialize, Deserialize)]
pub struct RemoteLogConfig {
    pub enable: bool,
    pub port: u32,
}

#[derive(Serialize, Deserialize)]
pub enum WifiAuthMethod {
    #[serde(rename = "WPA2_PERSONAL")]
    Personal,
    #[serde(rename = "WPA2_ENTERPRISE")]
    Enterprise,
}

impl Display for WifiAuthMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WifiAuthMethod::Personal => write!(f, "WPA2_PERSONAL"),
            WifiAuthMethod::Enterprise => write!(f, "WPA2_ENTERPRISE"),
        }
    }
}
