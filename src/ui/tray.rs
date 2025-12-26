use tray_icon::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    TrayIconBuilder,
};

pub fn create_tray_menu_with_servers(
    settings_item: &MenuItem,
    quit_item: &MenuItem,
) -> Menu {
    // Create tray menu
    let tray_menu = Menu::new();
    
    // Add running servers section
    let running_servers = crate::xray_manager::get_running_servers();
    if !running_servers.is_empty() {
        // Get server names from global VPN_SERVERS
        if let Ok(global_servers) = crate::vpn::VPN_SERVERS.lock() {
            if let Some(servers) = global_servers.as_ref() {
                for server in servers {
                    let server_key = server.get_server_key();
                    if running_servers.contains(&server_key) {
                        let status_text = format!("✓ {} ({}:{})", server.name, server.proxy_type, server.local_port);
                        let server_item = MenuItem::new(status_text, false, None);
                        tray_menu.append(&server_item).unwrap();
                    }
                }
            }
        }
        tray_menu.append(&PredefinedMenuItem::separator()).unwrap();
    }
    
    // Append settings and quit items
    tray_menu.append_items(&[
        settings_item,
        &PredefinedMenuItem::separator(),
        quit_item,
    ]).unwrap();

    tray_menu
}

pub fn create_tray_icon_with_servers(
    settings_item: &MenuItem,
    quit_item: &MenuItem,
) -> tray_icon::TrayIcon {
    // Create menu
    let tray_menu = create_tray_menu_with_servers(settings_item, quit_item);

    // Create icon (32x32 red square)
    let icon = create_icon();

    // Create tray icon with context menu
    TrayIconBuilder::new()
        .with_menu(Box::new(tray_menu))
        .with_tooltip("VPN Manager")
        .with_icon(icon)
        .build()
        .unwrap()
}

pub fn create_tray_icon(
    settings_item: &MenuItem,
    quit_item: &MenuItem,
) -> tray_icon::TrayIcon {
    // Create tray menu
    let tray_menu = Menu::new();
    
    // Append items to menu
    tray_menu.append_items(&[
        settings_item,
        &PredefinedMenuItem::separator(),
        quit_item,
    ]).unwrap();

    // Create icon (32x32 red square)
    let icon = create_icon();

    // Create tray icon with context menu
    TrayIconBuilder::new()
        .with_menu(Box::new(tray_menu))
        .with_tooltip("VPN Manager")
        .with_icon(icon)
        .build()
        .unwrap()
}

fn create_icon() -> tray_icon::Icon {
    // Create yellow star icon 32x32
    let width = 32;
    let height = 32;
    let mut rgba = Vec::with_capacity((width * height * 4) as usize);

    // Define colors
    let bg = [0, 0, 0, 0];           // Transparent background
    let star = [255, 215, 0, 255];   // Gold/Yellow
    let border = [218, 165, 32, 255]; // Darker gold

    let cx = 16.0;
    let cy = 16.0;

    for y in 0..height {
        for x in 0..width {
            let px = x as f32;
            let py = y as f32;
            
            // Calculate angle and distance from center
            let dx = px - cx;
            let dy = py - cy;
            let angle = dy.atan2(dx);
            let dist = (dx * dx + dy * dy).sqrt();
            
            // 5-pointed star calculation
            // Star has 5 points, so we check angle modulo (2π/5)
            let point_angle = (angle + std::f32::consts::PI * 2.5) % (std::f32::consts::PI * 2.0 / 5.0);
            let normalized = point_angle / (std::f32::consts::PI * 2.0 / 5.0);
            
            // Star shape: outer radius at points, inner radius between points
            let outer_radius = 12.0;
            let inner_radius = 5.0;
            
            // Calculate radius at this angle (sine wave between inner and outer)
            let target_radius = if normalized < 0.5 {
                inner_radius + (outer_radius - inner_radius) * (normalized * 2.0)
            } else {
                outer_radius - (outer_radius - inner_radius) * ((normalized - 0.5) * 2.0)
            };
            
            // Check if point is inside star
            let is_star = dist <= target_radius;
            let is_border = dist <= target_radius + 0.8 && dist > target_radius - 0.5;
            
            if is_star && !is_border {
                rgba.extend_from_slice(&star);
            } else if is_border {
                rgba.extend_from_slice(&border);
            } else {
                rgba.extend_from_slice(&bg);
            }
        }
    }

    tray_icon::Icon::from_rgba(rgba, width, height).expect("Failed to create icon")
}
