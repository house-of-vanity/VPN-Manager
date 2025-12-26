#![windows_subsystem = "windows"] // Commented out for debugging

mod ui;
mod vpn;
mod config;
mod xray_manager;

use tray_icon::menu::{MenuEvent, MenuItem};
use tray_icon::TrayIcon;
use std::sync::{Arc, Mutex, LazyLock};
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(windows)]
use windows::{
    Win32::{
        Foundation::HWND,
        UI::WindowsAndMessaging::*,
    },
};

// Global tokio runtime for async operations
pub static TOKIO_RUNTIME: LazyLock<tokio::runtime::Runtime> = LazyLock::new(|| {
    tokio::runtime::Runtime::new().expect("Failed to create tokio runtime")
});

// Flag to trigger menu update
pub static MENU_UPDATE_REQUESTED: AtomicBool = AtomicBool::new(false);

/// Request menu update (can be called from any thread)
pub fn request_menu_update() {
    MENU_UPDATE_REQUESTED.store(true, Ordering::Relaxed);
}

/// Restart all xray servers based on current config
/// This stops all running servers and starts enabled ones
pub fn restart_xray_servers() {
    // Stop all running servers first
    TOKIO_RUNTIME.block_on(async {
        let _ = xray_manager::stop_all_servers().await;
    });
    
    // Load config and start enabled servers
    if let Ok(config) = config::Config::load() {
        if !config.subscription_url.is_empty() && !config.xray_binary_path.is_empty() {
            // Fetch subscription URIs synchronously
            let subscription_uris = vpn::fetch_subscription_uris(&config.subscription_url);
            let mut servers = vpn::fetch_and_process_vpn_list(&config.subscription_url);
            vpn::assign_local_ports(&mut servers, &config.server_settings);
            
            // Update global VPN_SERVERS state
            if let Ok(mut global_servers) = vpn::VPN_SERVERS.lock() {
                *global_servers = Some(servers.clone());
            }
            
            // Start enabled servers
            TOKIO_RUNTIME.block_on(async {
                for server in servers.iter() {
                    if server.enabled {
                        let server_key = server.get_server_key();
                        
                        if let Some(settings) = config.server_settings.get(&server_key) {
                            if let Some(uri) = subscription_uris.get(&server_key) {
                                match xray_manager::start_server(
                                    &server_key,
                                    uri,
                                    settings.local_port,
                                    &settings.proxy_type,
                                    &config.xray_binary_path,
                                ).await {
                                    Ok(_) => println!("Started server: {}", server.name),
                                    Err(e) => eprintln!("Failed to start server {}: {}", server.name, e),
                                }
                            }
                        }
                    }
                }
            });
        }
    }
    
    // Request menu update
    request_menu_update();
}

/// Update tray icon menu with current running servers
pub fn update_tray_menu(tray_icon: &mut TrayIcon, settings_item: &MenuItem, quit_item: &MenuItem) {
    let new_menu = ui::create_tray_menu_with_servers(settings_item, quit_item);
    tray_icon.set_menu(Some(Box::new(new_menu)));
}

fn main() {
    // Enable DPI awareness at process start
    #[cfg(windows)]
    unsafe {
        use windows::Win32::UI::HiDpi::SetProcessDpiAwarenessContext;
        let _ = SetProcessDpiAwarenessContext(
            windows::Win32::UI::HiDpi::DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2
        );
    }
    
    // Auto-start servers on first launch
    restart_xray_servers();
    
    // Create menu items
    let settings_item = MenuItem::new("Settings", true, None);
    let quit_item = MenuItem::new("Exit", true, None);
    
    // Create tray icon with running servers list
    let mut tray_icon = ui::create_tray_icon_with_servers(&settings_item, &quit_item);

    // Event handling
    let menu_channel = MenuEvent::receiver();

    // Shared state for settings window
    #[cfg(windows)]
    let settings_window: Arc<Mutex<Option<HWND>>> = Arc::new(Mutex::new(None));

    // Windows message loop
    #[cfg(windows)]
    {
        let settings_window_clone = settings_window.clone();
        
        unsafe {
            let mut msg = MSG::default();
            
            // Process Windows messages
            loop {
                // Check if menu update requested
                if MENU_UPDATE_REQUESTED.load(Ordering::Relaxed) {
                    update_tray_menu(&mut tray_icon, &settings_item, &quit_item);
                    MENU_UPDATE_REQUESTED.store(false, Ordering::Relaxed);
                }
                
                // Check for menu events first
                if let Ok(event) = menu_channel.try_recv() {
                    if event.id == settings_item.id() {
                        // Open or focus settings window
                        let mut window = settings_window_clone.lock().unwrap();
                        if let Some(hwnd) = *window {
                            // Window already exists, bring it to front
                            if IsWindow(hwnd).as_bool() {
                                let _ = ShowWindow(hwnd, SW_RESTORE);
                                let _ = SetForegroundWindow(hwnd);
                            } else {
                                // Window was closed, create new one
                                *window = Some(ui::create_settings_window());
                            }
                        } else {
                            // Create new settings window
                            *window = Some(ui::create_settings_window());
                        }
                    } else if event.id == quit_item.id() {
                        // Stop all xray processes before exit
                        TOKIO_RUNTIME.block_on(async {
                            let _ = xray_manager::stop_all_servers().await;
                        });
                        break;
                    }
                }
                
                // Check if we need to update tray menu (poll for changes)
                // This is not ideal but tray-icon doesn't support callbacks
                // In a real app, you'd use a channel or event system
                
                // Process Windows messages
                let result = GetMessageW(&mut msg, None, 0, 0);
                if result.0 == 0 {
                    // WM_QUIT received
                    break;
                }
                
                if result.0 > 0 {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
        }
    }
}
