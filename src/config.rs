use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerSettings {
    pub local_port: u16,
    pub proxy_type: String, // "SOCKS" or "HTTP"
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub subscription_url: String,
    pub xray_binary_path: String,
    #[serde(default)]
    pub server_settings: HashMap<String, ServerSettings>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            subscription_url: String::new(),
            xray_binary_path: String::new(),
            server_settings: HashMap::new(),
        }
    }
}

impl Config {
    /// Get the config file path in AppData
    pub fn get_config_path() -> Result<PathBuf, String> {
        // Get AppData\Roaming path
        let appdata = std::env::var("APPDATA")
            .map_err(|_| "Failed to get APPDATA environment variable".to_string())?;
        
        let mut config_dir = PathBuf::from(appdata);
        config_dir.push("win-test-tray");
        
        // Create directory if it doesn't exist
        if !config_dir.exists() {
            fs::create_dir_all(&config_dir)
                .map_err(|e| format!("Failed to create config directory: {}", e))?;
        }
        
        config_dir.push("config.json");
        Ok(config_dir)
    }
    
    /// Load config from AppData
    pub fn load() -> Result<Config, String> {
        let config_path = Self::get_config_path()?;
        
        if !config_path.exists() {
            return Ok(Config::default());
        }
        
        let content = fs::read_to_string(&config_path)
            .map_err(|e| format!("Failed to read config file: {}", e))?;
        
        let config: Config = serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse config JSON: {}", e))?;
        
        Ok(config)
    }
    
    /// Save config to AppData
    pub fn save(&self) -> Result<(), String> {
        let config_path = Self::get_config_path()?;
        
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize config: {}", e))?;
        
        fs::write(&config_path, json)
            .map_err(|e| format!("Failed to write config file: {}", e))?;
        
        Ok(())
    }
}
