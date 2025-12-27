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
    #[serde(default)]
    pub autostart: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            subscription_url: String::new(),
            xray_binary_path: String::new(),
            server_settings: HashMap::new(),
            autostart: false,
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
        config_dir.push("Xray-VPN-Manager");
        
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
    
    /// Set autostart in Windows registry
    pub fn set_autostart(enabled: bool) -> Result<(), String> {
        #[cfg(windows)]
        {
            use windows::{
                core::w,
                Win32::System::Registry::{
                    RegOpenKeyExW, RegSetValueExW, RegDeleteKeyValueW,
                    HKEY_CURRENT_USER, KEY_WRITE, REG_SZ,
                },
            };
            
            let key_path = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
            let value_name = w!("Xray-VPN-Manager");
            
            unsafe {
                let mut hkey = Default::default();
                let result = RegOpenKeyExW(HKEY_CURRENT_USER, key_path, 0, KEY_WRITE, &mut hkey);
                if result.is_err() {
                    return Err(format!("Failed to open registry key: {:?}", result));
                }
                
                if enabled {
                    // Get current executable path
                    let exe_path = std::env::current_exe()
                        .map_err(|e| format!("Failed to get exe path: {}", e))?;
                    let exe_path_str = exe_path.to_string_lossy().to_string();
                    
                    let path_wide: Vec<u16> = format!("{}\0", exe_path_str).encode_utf16().collect();
                    let data = std::slice::from_raw_parts(
                        path_wide.as_ptr() as *const u8,
                        path_wide.len() * 2
                    );
                    
                    let result = RegSetValueExW(
                        hkey,
                        value_name,
                        0,
                        REG_SZ,
                        Some(data),
                    );
                    if result.is_err() {
                        return Err(format!("Failed to set registry value: {:?}", result));
                    }
                } else {
                    // Remove from autostart
                    let _ = RegDeleteKeyValueW(hkey, None, value_name);
                }
            }
        }
        
        #[cfg(not(windows))]
        {
            return Err("Autostart only supported on Windows".to_string());
        }
        
        Ok(())
    }
}
