use std::sync::Mutex;
use std::collections::HashSet;
use serde::{Deserialize, Serialize};

// Global state for VPN servers
pub static VPN_SERVERS: Mutex<Option<Vec<VpnServer>>> = Mutex::new(None);

// VPN server information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VpnServer {
    pub protocol: String,
    pub address: String,
    pub port: u16,
    pub name: String,
    pub enabled: bool,
    pub local_port: u16, // User-defined local port
    pub proxy_type: String, // "HTTP" or "SOCKS"
}

impl VpnServer {
    /// Get unique server key for stable identification
    pub fn get_server_key(&self) -> String {
        format!("{}://{}:{}", self.protocol, self.address, self.port)
    }
}

// Assign local ports to servers, preserving saved settings from config
pub fn assign_local_ports(servers: &mut [VpnServer], saved_settings: &std::collections::HashMap<String, crate::config::ServerSettings>) {
    let mut used_ports = HashSet::new();
    
    // First pass: assign saved settings (port + proxy type + enabled)
    for server in servers.iter_mut() {
        let key = server.get_server_key();
        if let Some(settings) = saved_settings.get(&key) {
            server.local_port = settings.local_port;
            server.proxy_type = settings.proxy_type.clone();
            server.enabled = settings.enabled;
            used_ports.insert(settings.local_port);
        }
    }
    
    // Second pass: assign new ports to servers without saved settings
    let mut next_port = 1080;
    for server in servers.iter_mut() {
        if server.local_port == 0 { // Not assigned yet
            while used_ports.contains(&next_port) {
                next_port += 1;
            }
            server.local_port = next_port;
            used_ports.insert(next_port);
            next_port += 1;
        }
    }
}

// Fetch and process VPN subscription list
pub fn fetch_and_process_vpn_list(url: &str) -> Vec<VpnServer> {
    let mut servers = Vec::new();
    
    // Fetch content from URL
    match reqwest::blocking::get(url) {
        Ok(response) => {
            match response.text() {
                Ok(content) => {
                    // Decode from base64
                    match base64::Engine::decode(&base64::engine::general_purpose::STANDARD, content.trim()) {
                        Ok(decoded_bytes) => {
                            match String::from_utf8(decoded_bytes) {
                                Ok(decoded_text) => {
                                    // Parse VPN links
                                    for line in decoded_text.lines() {
                                        let trimmed = line.trim();
                                        if let Some(server) = parse_vpn_uri(trimmed) {
                                            servers.push(server);
                                        }
                                    }
                                }
                                Err(_) => {}
                            }
                        }
                        Err(_) => {}
                    }
                }
                Err(_) => {}
            }
        }
        Err(_) => {}
    }
    
    servers
}

// Fetch subscription and return HashMap of server_key -> original_uri
pub fn fetch_subscription_uris(url: &str) -> std::collections::HashMap<String, String> {
    let mut uris = std::collections::HashMap::new();
    
    // Fetch content from URL
    match reqwest::blocking::get(url) {
        Ok(response) => {
            match response.text() {
                Ok(content) => {
                    // Decode from base64
                    match base64::Engine::decode(&base64::engine::general_purpose::STANDARD, content.trim()) {
                        Ok(decoded_bytes) => {
                            match String::from_utf8(decoded_bytes) {
                                Ok(decoded_text) => {
                                    // Parse VPN links
                                    for line in decoded_text.lines() {
                                        let trimmed = line.trim();
                                        if let Some(server) = parse_vpn_uri(trimmed) {
                                            let key = server.get_server_key();
                                            uris.insert(key, trimmed.to_string());
                                        }
                                    }
                                }
                                Err(_) => {}
                            }
                        }
                        Err(_) => {}
                    }
                }
                Err(_) => {}
            }
        }
        Err(_) => {}
    }
    
    uris
}

// Parse VPN URI using v2parser (supports vless, vmess, trojan, shadowsocks, socks)
fn parse_vpn_uri(uri: &str) -> Option<VpnServer> {
    // Check if it's a supported protocol
    let is_supported = uri.starts_with("vless://") 
        || uri.starts_with("vmess://") 
        || uri.starts_with("trojan://")
        || uri.starts_with("ss://")
        || uri.starts_with("shadowsocks://")
        || uri.starts_with("socks://");
    
    if !is_supported {
        return None;
    }
    
    // Use v2parser to get metadata
    match std::panic::catch_unwind(|| v2parser::parser::get_metadata(uri)) {
        Ok(metadata_json) => {
            if let Ok(metadata) = serde_json::from_str::<serde_json::Value>(&metadata_json) {
                let protocol = metadata["protocol"].as_str()?.to_uppercase();
                let address = metadata["address"].as_str()?.to_string();
                let port = metadata["port"].as_u64()? as u16;
                let name = metadata["name"].as_str().unwrap_or("Unnamed").to_string();
                
                Some(VpnServer {
                    protocol,
                    address,
                    port,
                    name,
                    enabled: false, // Default to disabled, will be enabled from config
                    local_port: 0, // Will be assigned by assign_local_ports
                    proxy_type: "SOCKS".to_string(), // Default to SOCKS
                })
            } else {
                None
            }
        }
        Err(_) => None,
    }
}
