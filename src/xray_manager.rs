use std::collections::HashMap;
use std::sync::{Mutex, LazyLock};
use v2parser::xray_runner::XrayRunner;
use v2parser::parser;

// Global state for running xray processes
pub static XRAY_PROCESSES: LazyLock<Mutex<HashMap<String, XrayRunner>>> = 
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Start xray server for a specific VPN server
/// Returns Ok if successful
pub async fn start_server(
    server_key: &str,
    uri: &str,
    local_port: u16,
    proxy_type: &str,
    xray_binary_path: &str,
) -> Result<(), String> {
    // Determine ports based on proxy type
    let (socks_port, http_port) = match proxy_type {
        "SOCKS" => (Some(local_port), None),
        "HTTP" => (None, Some(local_port)),
        _ => (Some(local_port), None), // Default to SOCKS
    };
    
    // Generate xray config from URI
    let config_json = parser::create_json_config(uri, socks_port, http_port);
    
    // Create and start xray runner
    let mut runner = XrayRunner::new();
    runner.start(&config_json, xray_binary_path)
        .await
        .map_err(|e| format!("Failed to start xray: {}", e))?;
    
    // Store runner in global state
    if let Ok(mut processes) = XRAY_PROCESSES.lock() {
        processes.insert(server_key.to_string(), runner);
    }
    
    Ok(())
}

/// Stop xray server for a specific server
pub async fn stop_server(server_key: &str) -> Result<(), String> {
    if let Ok(mut processes) = XRAY_PROCESSES.lock() {
        if let Some(mut runner) = processes.remove(server_key) {
            runner.stop()
                .await
                .map_err(|e| format!("Failed to stop xray: {}", e))?;
        }
    }
    Ok(())
}

/// Stop all running xray servers
pub async fn stop_all_servers() -> Result<(), String> {
    if let Ok(mut processes) = XRAY_PROCESSES.lock() {
        for (_key, mut runner) in processes.drain() {
            let _ = runner.stop().await; // Ignore errors during bulk shutdown
        }
    }
    Ok(())
}

/// Get list of running server keys
pub fn get_running_servers() -> Vec<String> {
    if let Ok(processes) = XRAY_PROCESSES.lock() {
        processes.keys().cloned().collect()
    } else {
        Vec::new()
    }
}
