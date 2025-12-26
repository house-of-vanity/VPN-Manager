#[cfg(windows)]
use windows::{
    core::{PCWSTR, w},
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, WPARAM, HINSTANCE, RECT, BOOL},
        Graphics::Gdi::{UpdateWindow, HBRUSH, SetBkMode, TRANSPARENT, HDC, GetStockObject, WHITE_BRUSH},
        System::LibraryLoader::GetModuleHandleW,
        UI::WindowsAndMessaging::*,
    },
};

use crate::vpn::{VpnServer, VPN_SERVERS, fetch_and_process_vpn_list, assign_local_ports};

// Custom Windows message for updating server list
const WM_UPDATE_SERVERS: u32 = WM_USER + 1;

// Control ID ranges
const ID_URL_EDIT: i32 = 1001;
const ID_UPDATE_BUTTON: i32 = 1002;
const ID_SCROLL_CONTAINER: i32 = 1003;
const ID_SCROLL_CONTAINER_CLASS: i32 = 1004; // Custom class for container
const ID_SAVE_BUTTON: i32 = 1005;
const ID_CANCEL_BUTTON: i32 = 1006;
const ID_XRAY_PATH_EDIT: i32 = 1007;
const ID_XRAY_BROWSE_BUTTON: i32 = 1008;
const ID_SERVER_CHECKBOX_BASE: i32 = 2000;  // 2000, 2001, 2002...
const ID_SERVER_PORT_EDIT_BASE: i32 = 3000; // 3000, 3001, 3002...
const ID_SERVER_PROXY_COMBO_BASE: i32 = 4000; // 4000, 4001, 4002...
const ID_SERVER_LABEL_BASE: i32 = 5000; // 5000, 5001, 5002... for "Proxy Port:" labels

// Layout constants for consistent formatting
const MARGIN: i32 = 15;
const FONT_SIZE: i32 = 32; // Reduced from 40
const LABEL_HEIGHT: i32 = 45; // Reduced from 50
const CONTROL_HEIGHT: i32 = 45; // Reduced from 50
const ROW_HEIGHT: i32 = 55; // Reduced from 60
const URL_LABEL_WIDTH: i32 = 200;

// Windows notification codes
const EN_CHANGE: usize = 0x0300;
const CBN_SELCHANGE: usize = 1;
const BN_CLICKED: usize = 0;

#[cfg(windows)]
pub unsafe fn create_settings_window() -> HWND {
    // Convert strings to UTF-16 (wide chars) for Windows API
    let class_name_str: Vec<u16> = "SettingsWindowClass\0"
        .encode_utf16()
        .collect();
    let class_name = PCWSTR::from_raw(class_name_str.as_ptr());
    
    // Register window class with white background
    let hinstance = unsafe { GetModuleHandleW(None).unwrap() };
    
    let wc = WNDCLASSW {
        lpfnWndProc: Some(settings_window_proc),
        hInstance: hinstance.into(),
        lpszClassName: class_name,
        hbrBackground: unsafe { HBRUSH(GetStockObject(WHITE_BRUSH).0) },
        style: CS_HREDRAW | CS_VREDRAW,
        ..Default::default()
    };
    
    unsafe { RegisterClassW(&wc) };
    
    // Register custom class for scroll container with white background
    let container_class_str: Vec<u16> = "ScrollContainerClass\0".encode_utf16().collect();
    let container_class = PCWSTR::from_raw(container_class_str.as_ptr());
    
    let wc_container = WNDCLASSW {
        lpfnWndProc: Some(container_window_proc),
        hInstance: hinstance.into(),
        lpszClassName: container_class,
        hbrBackground: unsafe { HBRUSH(GetStockObject(WHITE_BRUSH).0) },
        style: CS_HREDRAW | CS_VREDRAW,
        ..Default::default()
    };
    
    unsafe { RegisterClassW(&wc_container) };
    
    let window_title_str: Vec<u16> = "Settings\0"
        .encode_utf16()
        .collect();
    let window_title = PCWSTR::from_raw(window_title_str.as_ptr());
    
    // Create main window
    let hwnd = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            class_name,
            window_title,
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            900,
            1200, // Increased from 1050 to 1200 (+15%)
            None,
            None,
            hinstance,
            None,
        ).expect("Failed to create window")
    };
    
    println!("Settings window created: {:?}", hwnd);
    
    // Create controls
    unsafe { create_controls(hwnd, hinstance.into()) };
    
    unsafe {
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = UpdateWindow(hwnd);
    }
    
    hwnd
}

// Window procedure for scroll container to handle background and scrolling
#[cfg(windows)]
unsafe extern "system" fn container_window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_COMMAND => {
            // Forward WM_COMMAND to parent window
            unsafe {
                if let Ok(parent) = GetParent(hwnd) {
                    return SendMessageW(parent, msg, wparam, lparam);
                }
            }
            LRESULT(0)
        }
        WM_CTLCOLORSTATIC => {
            unsafe {
                let hdc = HDC(wparam.0 as *mut _);
                SetBkMode(hdc, TRANSPARENT);
                LRESULT(GetStockObject(WHITE_BRUSH).0 as isize)
            }
        }
        WM_VSCROLL => {
            // Handle vertical scrolling
            let action = wparam.0 & 0xFFFF;
            
            unsafe {
                let mut si = SCROLLINFO {
                    cbSize: std::mem::size_of::<SCROLLINFO>() as u32,
                    fMask: SIF_ALL,
                    nMin: 0,
                    nMax: 0,
                    nPage: 0,
                    nPos: 0,
                    nTrackPos: 0,
                };
                
                use windows::Win32::UI::WindowsAndMessaging::GetScrollInfo;
                let _ = GetScrollInfo(hwnd, SB_VERT, &mut si);
                
                let old_pos = si.nPos;
                
                match action {
                    0 => si.nPos -= 20, // SB_LINEUP
                    1 => si.nPos += 20, // SB_LINEDOWN
                    2 => si.nPos -= si.nPage as i32, // SB_PAGEUP
                    3 => si.nPos += si.nPage as i32, // SB_PAGEDOWN
                    5 => si.nPos = si.nTrackPos, // SB_THUMBTRACK
                    _ => {}
                }
                
                // Clamp position
                si.nPos = si.nPos.max(si.nMin);
                si.nPos = si.nPos.min(si.nMax - si.nPage as i32 + 1);
                
                if si.nPos != old_pos {
                    use windows::Win32::UI::Controls::SetScrollInfo;
                    si.fMask = SIF_POS;
                    SetScrollInfo(hwnd, SB_VERT, &si, true);
                    
                    // Move child windows based on scroll position
                    let scroll_delta = old_pos - si.nPos;
                    
                    // Enumerate and move all child windows
                    unsafe extern "system" fn move_child(child: HWND, lparam: LPARAM) -> BOOL {
                        let delta = lparam.0 as i32;
                        let mut rect = RECT::default();
                        unsafe {
                            if GetWindowRect(child, &mut rect).is_ok() {
                                let parent = GetParent(child).unwrap_or(HWND::default());
                                let mut pt = windows::Win32::Foundation::POINT { x: rect.left, y: rect.top };
                                use windows::Win32::Graphics::Gdi::ScreenToClient;
                                let _ = ScreenToClient(parent, &mut pt);
                                
                                let _ = SetWindowPos(
                                    child,
                                    None,
                                    pt.x,
                                    pt.y + delta,
                                    0, 0,
                                    SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
                                );
                            }
                        }
                        true.into()
                    }
                    
                    let _ = EnumChildWindows(hwnd, Some(move_child), LPARAM(scroll_delta as isize));
                }
            }
            LRESULT(0)
        }
        WM_MOUSEWHEEL => {
            // Handle mouse wheel scrolling
            let delta = ((wparam.0 >> 16) & 0xFFFF) as i16;
            let scroll_lines = if delta > 0 { 0 } else { 1 }; // 0=SB_LINEUP, 1=SB_LINEDOWN
            
            // Send scroll message multiple times for smoother scrolling
            for _ in 0..(delta.abs() / 40).max(1) {
                unsafe {
                    SendMessageW(hwnd, WM_VSCROLL, WPARAM(scroll_lines), LPARAM(0));
                }
            }
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

#[cfg(windows)]
unsafe fn create_controls(parent: HWND, hinstance: HINSTANCE) {
    // Load config and set URL field
    let config = crate::config::Config::load().unwrap_or_default();
    
    // Create font for all controls
    let hfont = unsafe {
        use windows::Win32::Graphics::Gdi::{CreateFontW, DEFAULT_CHARSET, OUT_DEFAULT_PRECIS, 
            CLIP_DEFAULT_PRECIS, DEFAULT_QUALITY, DEFAULT_PITCH, FF_DONTCARE, FW_NORMAL};
        CreateFontW(
            FONT_SIZE,
            0, 0, 0,
            FW_NORMAL.0 as i32,
            0, 0, 0,
            DEFAULT_CHARSET.0 as u32,
            OUT_DEFAULT_PRECIS.0 as u32,
            CLIP_DEFAULT_PRECIS.0 as u32,
            DEFAULT_QUALITY.0 as u32,
            (DEFAULT_PITCH.0 | FF_DONTCARE.0) as u32,
            w!("Segoe UI"),
        )
    };
    
    // First row Y position
    let row1_y = MARGIN;
    
    // Label "Subscription URL:"
    let label_text: Vec<u16> = "Subscription URL:\0".encode_utf16().collect();
    let label = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("STATIC"),
            PCWSTR::from_raw(label_text.as_ptr()),
            WS_CHILD | WS_VISIBLE,
            MARGIN,
            row1_y,
            URL_LABEL_WIDTH,
            LABEL_HEIGHT,
            parent,
            None,
            hinstance,
            None,
        ).ok()
    };
    if let Some(lbl) = label {
        unsafe { SendMessageW(lbl, WM_SETFONT, WPARAM(hfont.0 as usize), LPARAM(1)); }
    }
    
    // URL Edit control - set loaded URL
    let url_text_wide: Vec<u16> = format!("{}\0", config.subscription_url).encode_utf16().collect();
    let url_edit = unsafe {
        CreateWindowExW(
            WS_EX_CLIENTEDGE,
            w!("EDIT"),
            PCWSTR::from_raw(url_text_wide.as_ptr()),
            WS_CHILD | WS_VISIBLE | WS_BORDER | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
            MARGIN + URL_LABEL_WIDTH + 10,
            row1_y,
            450,
            CONTROL_HEIGHT,
            parent,
            HMENU(ID_URL_EDIT as _),
            hinstance,
            None,
        ).expect("Failed to create URL edit control")
    };
    unsafe { SendMessageW(url_edit, WM_SETFONT, WPARAM(hfont.0 as usize), LPARAM(1)); }
    
    // Update button
    let update_btn_text: Vec<u16> = "Update\0".encode_utf16().collect();
    let update_btn = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("BUTTON"),
            PCWSTR::from_raw(update_btn_text.as_ptr()),
            WS_CHILD | WS_VISIBLE | WINDOW_STYLE(BS_PUSHBUTTON as u32),
            MARGIN + URL_LABEL_WIDTH + 10 + 450 + 10,
            row1_y,
            120,
            CONTROL_HEIGHT,
            parent,
            HMENU(ID_UPDATE_BUTTON as _),
            hinstance,
            None,
        ).ok()
    };
    if let Some(btn) = update_btn {
        unsafe { SendMessageW(btn, WM_SETFONT, WPARAM(hfont.0 as usize), LPARAM(1)); }
    }
    
    // Second row Y position
    let row2_y = row1_y + CONTROL_HEIGHT + MARGIN;
    
    // Label "Xray Binary:"
    let xray_label: Vec<u16> = "Xray Binary:\0".encode_utf16().collect();
    let xray_lbl = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("STATIC"),
            PCWSTR::from_raw(xray_label.as_ptr()),
            WS_CHILD | WS_VISIBLE,
            MARGIN,
            row2_y,
            URL_LABEL_WIDTH,
            LABEL_HEIGHT,
            parent,
            None,
            hinstance,
            None,
        ).ok()
    };
    if let Some(lbl) = xray_lbl {
        unsafe { SendMessageW(lbl, WM_SETFONT, WPARAM(hfont.0 as usize), LPARAM(1)); }
    }
    
    // Xray binary path edit control
    let xray_path_text_wide: Vec<u16> = format!("{}\0", config.xray_binary_path).encode_utf16().collect();
    let xray_path_edit = unsafe {
        CreateWindowExW(
            WS_EX_CLIENTEDGE,
            w!("EDIT"),
            PCWSTR::from_raw(xray_path_text_wide.as_ptr()),
            WS_CHILD | WS_VISIBLE | WS_BORDER | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
            MARGIN + URL_LABEL_WIDTH + 10,
            row2_y,
            450,
            CONTROL_HEIGHT,
            parent,
            HMENU(ID_XRAY_PATH_EDIT as _),
            hinstance,
            None,
        ).expect("Failed to create Xray path edit control")
    };
    unsafe { SendMessageW(xray_path_edit, WM_SETFONT, WPARAM(hfont.0 as usize), LPARAM(1)); }
    
    // Browse button
    let browse_btn_text: Vec<u16> = "Browse...\0".encode_utf16().collect();
    let browse_btn = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("BUTTON"),
            PCWSTR::from_raw(browse_btn_text.as_ptr()),
            WS_CHILD | WS_VISIBLE | WINDOW_STYLE(BS_PUSHBUTTON as u32),
            MARGIN + URL_LABEL_WIDTH + 10 + 450 + 10,
            row2_y,
            120,
            CONTROL_HEIGHT,
            parent,
            HMENU(ID_XRAY_BROWSE_BUTTON as _),
            hinstance,
            None,
        ).ok()
    };
    if let Some(btn) = browse_btn {
        unsafe { SendMessageW(btn, WM_SETFONT, WPARAM(hfont.0 as usize), LPARAM(1)); }
    }
    
    // Third row Y position
    let row3_y = row2_y + CONTROL_HEIGHT + MARGIN;
    
    // Label "VPN Servers:"
    let list_label: Vec<u16> = "VPN Servers:\0".encode_utf16().collect();
    let list_lbl = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("STATIC"),
            PCWSTR::from_raw(list_label.as_ptr()),
            WS_CHILD | WS_VISIBLE,
            MARGIN,
            row3_y,
            200,
            LABEL_HEIGHT,
            parent,
            None,
            hinstance,
            None,
        ).ok()
    };
    if let Some(lbl) = list_lbl {
        unsafe { SendMessageW(lbl, WM_SETFONT, WPARAM(hfont.0 as usize), LPARAM(1)); }
    }
    
    // Server list container Y position
    let container_y = row3_y + LABEL_HEIGHT + 10;
    
    // Get client area size to calculate container height dynamically
    let mut client_rect = RECT::default();
    unsafe { GetClientRect(parent, &mut client_rect).ok() };
    let client_width = client_rect.right - client_rect.left;
    let client_height = client_rect.bottom - client_rect.top;
    
    // Calculate container size based on window size
    // Reserve space for Save/Cancel buttons at the bottom (60px)
    const BUTTON_ROW_HEIGHT: i32 = 60;
    let container_width = client_width - (2 * MARGIN);
    let container_height = client_height - container_y - MARGIN - BUTTON_ROW_HEIGHT;
    
    // Scrollable container for server panels with custom class
    let container_class_str: Vec<u16> = "ScrollContainerClass\0".encode_utf16().collect();
    unsafe {
        CreateWindowExW(
            WS_EX_CLIENTEDGE,
            PCWSTR::from_raw(container_class_str.as_ptr()),
            PCWSTR::null(),
            WS_CHILD | WS_VISIBLE | WS_VSCROLL,
            MARGIN,
            container_y,
            container_width,
            container_height,
            parent,
            HMENU(ID_SCROLL_CONTAINER as _),
            hinstance,
            None,
        ).expect("Failed to create scroll container")
    };
    
    // Bottom buttons row
    let buttons_y = container_y + container_height + 10;
    
    // Save button
    let save_btn_text: Vec<u16> = "Save\0".encode_utf16().collect();
    let save_btn = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("BUTTON"),
            PCWSTR::from_raw(save_btn_text.as_ptr()),
            WS_CHILD | WS_VISIBLE | WINDOW_STYLE(BS_PUSHBUTTON as u32),
            client_width - 240, // Right side: 120px button + 10px margin + 120px button
            buttons_y,
            110,
            CONTROL_HEIGHT,
            parent,
            HMENU(ID_SAVE_BUTTON as _),
            hinstance,
            None,
        ).ok()
    };
    if let Some(btn) = save_btn {
        unsafe { SendMessageW(btn, WM_SETFONT, WPARAM(hfont.0 as usize), LPARAM(1)); }
    }
    
    // Cancel button
    let cancel_btn_text: Vec<u16> = "Cancel\0".encode_utf16().collect();
    let cancel_btn = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("BUTTON"),
            PCWSTR::from_raw(cancel_btn_text.as_ptr()),
            WS_CHILD | WS_VISIBLE | WINDOW_STYLE(BS_PUSHBUTTON as u32),
            client_width - 120, // Right side
            buttons_y,
            110,
            CONTROL_HEIGHT,
            parent,
            HMENU(ID_CANCEL_BUTTON as _),
            hinstance,
            None,
        ).ok()
    };
    if let Some(btn) = cancel_btn {
        unsafe { SendMessageW(btn, WM_SETFONT, WPARAM(hfont.0 as usize), LPARAM(1)); }
    }
    
    // Auto-load servers from subscription URL if available
    if !config.subscription_url.is_empty() {
        let url = config.subscription_url.clone();
        let saved_settings = config.server_settings.clone();
        let hwnd_raw = parent.0 as isize;
        
        std::thread::spawn(move || {
            let mut servers = fetch_and_process_vpn_list(&url);
            
            // Assign settings (preserving saved ones)
            assign_local_ports(&mut servers, &saved_settings);
            
            // Store servers globally
            if let Ok(mut global_servers) = VPN_SERVERS.lock() {
                *global_servers = Some(servers.clone());
            }
            
            // Update UI on main thread via PostMessage
            unsafe {
                let hwnd = HWND(hwnd_raw as *mut _);
                let _ = PostMessageW(hwnd, WM_UPDATE_SERVERS, WPARAM(0), LPARAM(0));
            }
        });
    }
}

#[cfg(windows)]
unsafe extern "system" fn settings_window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_COMMAND => {
            let control_id = wparam.0 & 0xFFFF;
            let notification_code = (wparam.0 >> 16) & 0xFFFF;
            
            // Update button clicked
            if control_id == ID_UPDATE_BUTTON as usize && notification_code == 0 {
                println!("Update button clicked!");
                
                // Get text from edit control
                let url_edit = unsafe { GetDlgItem(hwnd, ID_URL_EDIT) };
                if url_edit.is_ok() && !url_edit.as_ref().unwrap().is_invalid() {
                    let mut buffer = vec![0u16; 2048];
                    let len = unsafe { 
                        GetWindowTextW(url_edit.unwrap(), &mut buffer) 
                    };
                    
                    if len > 0 {
                        let url = String::from_utf16_lossy(&buffer[..len as usize]);
                        println!("URL entered: {}", url);
                        
                        // Fetch and process in background thread
                        let hwnd_raw = hwnd.0 as isize;
                        std::thread::spawn(move || {
                            let mut servers = fetch_and_process_vpn_list(&url);
                            
                            // Load config to get saved settings
                            let config = crate::config::Config::load().unwrap_or_default();
                            
                            // Assign settings (preserving saved ones)
                            assign_local_ports(&mut servers, &config.server_settings);
                            
                            // Store servers globally
                            if let Ok(mut global_servers) = VPN_SERVERS.lock() {
                                *global_servers = Some(servers.clone());
                            }
                            
                            // Update UI on main thread via PostMessage
                            unsafe {
                                let hwnd = HWND(hwnd_raw as *mut _);
                                let _ = PostMessageW(hwnd, WM_UPDATE_SERVERS, WPARAM(0), LPARAM(0));
                            }
                        });
                    } else {
                        println!("No URL entered");
                    }
                }
            }
            // Handle checkbox changes
            else if control_id >= ID_SERVER_CHECKBOX_BASE as usize 
                    && control_id < ID_SERVER_PORT_EDIT_BASE as usize 
                    && notification_code == BN_CLICKED {
                let server_index = control_id - ID_SERVER_CHECKBOX_BASE as usize;
                
                // Get checkbox state
                if let Ok(container) = unsafe { GetDlgItem(hwnd, ID_SCROLL_CONTAINER) } {
                    if let Ok(checkbox) = unsafe { GetDlgItem(container, control_id as i32) } {
                        let state = unsafe { SendMessageW(checkbox, BM_GETCHECK, WPARAM(0), LPARAM(0)) };
                        let checked = state.0 == 1;
                        
                        // Update global state
                        if let Ok(mut global_servers) = VPN_SERVERS.try_lock() {
                            if let Some(servers) = global_servers.as_mut() {
                                if let Some(server) = servers.get_mut(server_index) {
                                    server.enabled = checked;
                                }
                            }
                        }
                    }
                }
            }
            // Handle port edit changes
            else if control_id >= ID_SERVER_PORT_EDIT_BASE as usize 
                    && control_id < ID_SERVER_PROXY_COMBO_BASE as usize
                    && notification_code == EN_CHANGE {
                let server_index = control_id - ID_SERVER_PORT_EDIT_BASE as usize;
                
                // Get container window first, then find edit control in container
                if let Ok(container) = unsafe { GetDlgItem(hwnd, ID_SCROLL_CONTAINER) } {
                    if let Ok(edit) = unsafe { GetDlgItem(container, control_id as i32) } {
                        let mut buffer = vec![0u16; 16];
                        let len = unsafe { GetWindowTextW(edit, &mut buffer) };
                        
                        if len > 0 {
                            let port_text = String::from_utf16_lossy(&buffer[..len as usize]);
                            
                            // Parse port as u16
                            if let Ok(port) = port_text.parse::<u16>() {
                                // Update global state
                                if let Ok(mut global_servers) = VPN_SERVERS.try_lock() {
                                    if let Some(servers) = global_servers.as_mut() {
                                        if let Some(server) = servers.get_mut(server_index) {
                                            server.local_port = port;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            // Handle proxy type combobox changes
            else if control_id >= ID_SERVER_PROXY_COMBO_BASE as usize 
                    && notification_code == CBN_SELCHANGE {
                let server_index = control_id - ID_SERVER_PROXY_COMBO_BASE as usize;
                
                // Get container window first, then find combo control in container
                if let Ok(container) = unsafe { GetDlgItem(hwnd, ID_SCROLL_CONTAINER) } {
                    if let Ok(combo) = unsafe { GetDlgItem(container, control_id as i32) } {
                        let sel_idx = unsafe { SendMessageW(combo, CB_GETCURSEL, WPARAM(0), LPARAM(0)) };
                        
                        if sel_idx.0 >= 0 {
                            let proxy_type = if sel_idx.0 == 0 { "SOCKS" } else { "HTTP" };
                            
                            // Update global state
                            if let Ok(mut global_servers) = VPN_SERVERS.try_lock() {
                                if let Some(servers) = global_servers.as_mut() {
                                    if let Some(server) = servers.get_mut(server_index) {
                                        server.proxy_type = proxy_type.to_string();
                                    }
                                }
                            }
                        }
                    }
                }
            }
            // Handle Browse button for Xray binary
            else if control_id == ID_XRAY_BROWSE_BUTTON as usize && notification_code == 0 {
                
                // Open file dialog
                use windows::Win32::UI::Shell::Common::COMDLG_FILTERSPEC;
                use windows::Win32::UI::Shell::{IFileOpenDialog, FileOpenDialog};
                use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_APARTMENTTHREADED};
                
                unsafe {
                    // Initialize COM
                    let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
                    
                    // Create file open dialog
                    if let Ok(dialog) = CoCreateInstance::<_, IFileOpenDialog>(&FileOpenDialog, None, CLSCTX_ALL) {
                        // Set file type filter
                        let filter_spec = [
                            COMDLG_FILTERSPEC {
                                pszName: w!("Executable Files"),
                                pszSpec: w!("*.exe"),
                            },
                            COMDLG_FILTERSPEC {
                                pszName: w!("All Files"),
                                pszSpec: w!("*.*"),
                            },
                        ];
                        
                        let _ = dialog.SetFileTypes(&filter_spec);
                        let _ = dialog.SetFileTypeIndex(1);
                        
                        // Show dialog
                        if dialog.Show(hwnd).is_ok() {
                            if let Ok(result) = dialog.GetResult() {
                                if let Ok(path) = result.GetDisplayName(windows::Win32::UI::Shell::SIGDN_FILESYSPATH) {
                                    let path_str = path.to_string().unwrap_or_default();
                                    
                                    // Update edit control
                                    if let Ok(xray_edit) = GetDlgItem(hwnd, ID_XRAY_PATH_EDIT) {
                                        let path_wide: Vec<u16> = format!("{}\0", path_str).encode_utf16().collect();
                                        SetWindowTextW(xray_edit, PCWSTR::from_raw(path_wide.as_ptr())).ok();
                                    }
                                }
                            }
                        }
                    }
                }
            }
            // Handle Save button
            else if control_id == ID_SAVE_BUTTON as usize && notification_code == 0 {
                
                // Get URL from edit control
                let url_edit = unsafe { GetDlgItem(hwnd, ID_URL_EDIT) };
                let subscription_url = if url_edit.is_ok() && !url_edit.as_ref().unwrap().is_invalid() {
                    let mut buffer = vec![0u16; 2048];
                    let len = unsafe { GetWindowTextW(url_edit.unwrap(), &mut buffer) };
                    if len > 0 {
                        String::from_utf16_lossy(&buffer[..len as usize])
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                };
                
                // Get Xray binary path from edit control
                let xray_edit = unsafe { GetDlgItem(hwnd, ID_XRAY_PATH_EDIT) };
                let xray_binary_path = if xray_edit.is_ok() && !xray_edit.as_ref().unwrap().is_invalid() {
                    let mut buffer = vec![0u16; 2048];
                    let len = unsafe { GetWindowTextW(xray_edit.unwrap(), &mut buffer) };
                    if len > 0 {
                        String::from_utf16_lossy(&buffer[..len as usize])
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                };
                
                // Build server_settings HashMap from current servers
                use std::collections::HashMap;
                let mut server_settings = HashMap::new();
                
                if let Ok(global_servers) = VPN_SERVERS.lock() {
                    if let Some(servers) = global_servers.as_ref() {
                        for server in servers {
                            let key = server.get_server_key();
                            let settings = crate::config::ServerSettings {
                                local_port: server.local_port,
                                proxy_type: server.proxy_type.clone(),
                                enabled: server.enabled,
                            };
                            server_settings.insert(key, settings);
                        }
                    }
                }
                
                // Create and save config
                let config = crate::config::Config {
                    subscription_url,
                    xray_binary_path,
                    server_settings,
                };
                
                match config.save() {
                    Ok(_) => {
                        // Restart xray servers with new config
                        crate::restart_xray_servers();
                        
                        // Close window
                        unsafe { let _ = DestroyWindow(hwnd); }
                    }
                    Err(e) => {
                        // Show error message box
                        let error_msg: Vec<u16> = format!("Failed to save config:\n{}\0", e).encode_utf16().collect();
                        let title: Vec<u16> = "Error\0".encode_utf16().collect();
                        unsafe {
                            MessageBoxW(
                                hwnd,
                                PCWSTR::from_raw(error_msg.as_ptr()),
                                PCWSTR::from_raw(title.as_ptr()),
                                MB_OK | MB_ICONERROR,
                            );
                        }
                    }
                }
            }
            // Handle Cancel button
            else if control_id == ID_CANCEL_BUTTON as usize && notification_code == 0 {
                // Close window without saving
                unsafe { let _ = DestroyWindow(hwnd); }
            }
            
            LRESULT(0)
        }
        WM_CTLCOLORSTATIC => {
            // Make static text background transparent
            unsafe {
                let hdc = HDC(wparam.0 as *mut _);
                SetBkMode(hdc, TRANSPARENT);
                // Return white brush to match window background
                LRESULT(GetStockObject(WHITE_BRUSH).0 as isize)
            }
        }
        WM_SIZE => {
            // Resize controls when window is resized
            let width = (lparam.0 & 0xFFFF) as i32;
            let height = ((lparam.0 >> 16) & 0xFFFF) as i32;
            
            const BUTTON_ROW_HEIGHT: i32 = 60;
            let row1_y = MARGIN;
            let row2_y = row1_y + CONTROL_HEIGHT + MARGIN;
            let row3_y = row2_y + CONTROL_HEIGHT + MARGIN;
            let container_y = row3_y + LABEL_HEIGHT + 10;
            let container_height = height - container_y - MARGIN - BUTTON_ROW_HEIGHT;
            let buttons_y = container_y + container_height + 10;
            
            unsafe {
                // Resize URL edit control
                if let Ok(url_edit) = GetDlgItem(hwnd, ID_URL_EDIT) {
                    if !url_edit.is_invalid() {
                        SetWindowPos(
                            url_edit,
                            None,
                            0, 0,
                            width - (MARGIN + URL_LABEL_WIDTH + 10 + 120 + 10 + MARGIN),
                            CONTROL_HEIGHT,
                            SWP_NOMOVE | SWP_NOZORDER,
                        ).ok();
                    }
                }
                
                // Move Update button to stay on the right
                if let Ok(update_btn) = GetDlgItem(hwnd, ID_UPDATE_BUTTON) {
                    if !update_btn.is_invalid() {
                        SetWindowPos(
                            update_btn,
                            None,
                            width - 120 - MARGIN,
                            row1_y,
                            0, 0,
                            SWP_NOSIZE | SWP_NOZORDER,
                        ).ok();
                    }
                }
                
                // Resize Xray path edit control
                if let Ok(xray_edit) = GetDlgItem(hwnd, ID_XRAY_PATH_EDIT) {
                    if !xray_edit.is_invalid() {
                        SetWindowPos(
                            xray_edit,
                            None,
                            0, 0,
                            width - (MARGIN + URL_LABEL_WIDTH + 10 + 120 + 10 + MARGIN),
                            CONTROL_HEIGHT,
                            SWP_NOMOVE | SWP_NOZORDER,
                        ).ok();
                    }
                }
                
                // Move Browse button to stay on the right
                if let Ok(browse_btn) = GetDlgItem(hwnd, ID_XRAY_BROWSE_BUTTON) {
                    if !browse_btn.is_invalid() {
                        SetWindowPos(
                            browse_btn,
                            None,
                            width - 120 - MARGIN,
                            row2_y,
                            0, 0,
                            SWP_NOSIZE | SWP_NOZORDER,
                        ).ok();
                    }
                }
                
                // Resize scroll container to fill remaining space
                if let Ok(container) = GetDlgItem(hwnd, ID_SCROLL_CONTAINER) {
                    if !container.is_invalid() {
                        SetWindowPos(
                            container,
                            None,
                            0, 0,
                            width - (2 * MARGIN),
                            container_height,
                            SWP_NOMOVE | SWP_NOZORDER,
                        ).ok();
                        
                        // Only resize checkboxes, don't rebuild entire list
                        resize_server_list_items(container);
                    }
                }
                
                // Move Save button
                if let Ok(save_btn) = GetDlgItem(hwnd, ID_SAVE_BUTTON) {
                    if !save_btn.is_invalid() {
                        SetWindowPos(
                            save_btn,
                            None,
                            width - 240,
                            buttons_y,
                            0, 0,
                            SWP_NOSIZE | SWP_NOZORDER,
                        ).ok();
                    }
                }
                
                // Move Cancel button
                if let Ok(cancel_btn) = GetDlgItem(hwnd, ID_CANCEL_BUTTON) {
                    if !cancel_btn.is_invalid() {
                        SetWindowPos(
                            cancel_btn,
                            None,
                            width - 120,
                            buttons_y,
                            0, 0,
                            SWP_NOSIZE | SWP_NOZORDER,
                        ).ok();
                    }
                }
            }
            LRESULT(0)
        }
        _ if msg == WM_UPDATE_SERVERS => {
            // Custom message: rebuild server list UI
            if let Ok(global_servers) = VPN_SERVERS.lock() {
                if let Some(servers) = &*global_servers {
                    unsafe {
                        rebuild_server_list(hwnd, servers);
                    }
                }
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            println!("Settings window destroyed");
            LRESULT(0)
        }
        WM_CLOSE => {
            unsafe { let _ = DestroyWindow(hwnd); };
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

// Rebuild the server list with custom panels
#[cfg(windows)]
unsafe fn rebuild_server_list(parent_hwnd: HWND, servers: &[VpnServer]) {
    let container = unsafe { GetDlgItem(parent_hwnd, ID_SCROLL_CONTAINER) };
    if container.is_err() || container.as_ref().unwrap().is_invalid() {
        return;
    }
    let container = container.unwrap();
    
    // Destroy all existing child windows in container
    unsafe {
        let _ = EnumChildWindows(
            container,
            Some(destroy_child_window),
            LPARAM(0),
        );
    }
    
    let hinstance: HINSTANCE = unsafe { GetModuleHandleW(None).unwrap().into() };
    
    // Create font for controls (reduced to match main font)
    let hfont = unsafe {
        use windows::Win32::Graphics::Gdi::{CreateFontW, DEFAULT_CHARSET, OUT_DEFAULT_PRECIS, 
            CLIP_DEFAULT_PRECIS, DEFAULT_QUALITY, DEFAULT_PITCH, FF_DONTCARE, FW_NORMAL};
        CreateFontW(
            30, // Reduced from 36
            0, 0, 0,
            FW_NORMAL.0 as i32,
            0, 0, 0,
            DEFAULT_CHARSET.0 as u32,
            OUT_DEFAULT_PRECIS.0 as u32,
            CLIP_DEFAULT_PRECIS.0 as u32,
            DEFAULT_QUALITY.0 as u32,
            (DEFAULT_PITCH.0 | FF_DONTCARE.0) as u32,
            w!("Segoe UI"),
        )
    };
    
    // Get container width for dynamic checkbox width
    let mut container_rect = RECT::default();
    unsafe { GetClientRect(container, &mut container_rect).ok() };
    let container_width = container_rect.right - container_rect.left;
    
    const SERVER_ITEM_MARGIN: i32 = 10;
    const LABEL_WIDTH: i32 = 130; // Increased from 100 to fit "Proxy Port:"
    const PORT_EDIT_WIDTH: i32 = 90; // Increased from 80
    const COMBO_WIDTH: i32 = 130; // Increased from 120
    const RIGHT_CONTROLS_WIDTH: i32 = LABEL_WIDTH + PORT_EDIT_WIDTH + COMBO_WIDTH + 30; // +30 for spacing
    let checkbox_width = container_width - RIGHT_CONTROLS_WIDTH - 20; // Dynamic width
    
    for (idx, server) in servers.iter().enumerate() {
        let y_pos = idx as i32 * ROW_HEIGHT + SERVER_ITEM_MARGIN;
        
        // Checkbox (enabled/disabled) - dynamic width
        let checkbox_text = format!("{} - {} ({}:{})\0", 
            server.name, server.address, server.protocol, server.port);
        let checkbox_text_wide: Vec<u16> = checkbox_text.encode_utf16().collect();
        
        let checkbox = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("BUTTON"),
                PCWSTR::from_raw(checkbox_text_wide.as_ptr()),
                WS_CHILD | WS_VISIBLE | WINDOW_STYLE(BS_AUTOCHECKBOX as u32),
                10,
                y_pos,
                checkbox_width,
                CONTROL_HEIGHT,
                container,
                HMENU((ID_SERVER_CHECKBOX_BASE + idx as i32) as _),
                hinstance,
                None,
            ).ok()
        };
        
        if let Some(cb) = checkbox {
            unsafe { SendMessageW(cb, WM_SETFONT, WPARAM(hfont.0 as usize), LPARAM(1)); }
        }
        
        // Set checkbox state
        if let Ok(checkbox) = unsafe { GetDlgItem(container, ID_SERVER_CHECKBOX_BASE + idx as i32) } {
            unsafe {
                SendMessageW(
                    checkbox,
                    BM_SETCHECK,
                    WPARAM(if server.enabled { 1 } else { 0 }),
                    LPARAM(0),
                );
            }
        }
        
        let right_controls_x = 10 + checkbox_width + 10;
        
        // Label "Proxy Port:" with ID for resizing
        let port_label: Vec<u16> = "Proxy Port:\0".encode_utf16().collect();
        let port_lbl = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("STATIC"),
                PCWSTR::from_raw(port_label.as_ptr()),
                WS_CHILD | WS_VISIBLE,
                right_controls_x,
                y_pos + 5,
                LABEL_WIDTH,
                CONTROL_HEIGHT,
                container,
                HMENU((ID_SERVER_LABEL_BASE + idx as i32) as _), // Add ID
                hinstance,
                None,
            ).ok()
        };
        if let Some(lbl) = port_lbl {
            unsafe { SendMessageW(lbl, WM_SETFONT, WPARAM(hfont.0 as usize), LPARAM(1)); }
        }
        
        // Edit control for proxy port
        let port_text_wide: Vec<u16> = format!("{}\0", server.local_port).encode_utf16().collect();
        let port_edit = unsafe {
            CreateWindowExW(
                WS_EX_CLIENTEDGE,
                w!("EDIT"),
                PCWSTR::from_raw(port_text_wide.as_ptr()),
                WS_CHILD | WS_VISIBLE | WS_BORDER | WINDOW_STYLE(ES_AUTOHSCROLL as u32 | ES_NUMBER as u32),
                right_controls_x + LABEL_WIDTH + 5,
                y_pos,
                PORT_EDIT_WIDTH,
                CONTROL_HEIGHT,
                container,
                HMENU((ID_SERVER_PORT_EDIT_BASE + idx as i32) as _),
                hinstance,
                None,
            ).ok()
        };
        if let Some(edit) = port_edit {
            unsafe { SendMessageW(edit, WM_SETFONT, WPARAM(hfont.0 as usize), LPARAM(1)); }
        }
        
        // ComboBox for proxy type (SOCKS/HTTP)
        let combo = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("COMBOBOX"),
                PCWSTR::null(),
                WS_CHILD | WS_VISIBLE | WINDOW_STYLE(CBS_DROPDOWNLIST as u32 | WS_VSCROLL.0),
                right_controls_x + LABEL_WIDTH + 5 + PORT_EDIT_WIDTH + 10,
                y_pos,
                COMBO_WIDTH,
                200, // Dropdown height
                container,
                HMENU((ID_SERVER_PROXY_COMBO_BASE + idx as i32) as _),
                hinstance,
                None,
            ).ok()
        };
        
        if let Some(cb) = combo {
            unsafe { SendMessageW(cb, WM_SETFONT, WPARAM(hfont.0 as usize), LPARAM(1)); }
            
            // Add items
            let socks_text: Vec<u16> = "SOCKS\0".encode_utf16().collect();
            let http_text: Vec<u16> = "HTTP\0".encode_utf16().collect();
            
            unsafe {
                SendMessageW(cb, CB_ADDSTRING, WPARAM(0), LPARAM(socks_text.as_ptr() as isize));
                SendMessageW(cb, CB_ADDSTRING, WPARAM(0), LPARAM(http_text.as_ptr() as isize));
                
                // Set current selection
                let sel_idx = if server.proxy_type == "SOCKS" { 0 } else { 1 };
                SendMessageW(cb, CB_SETCURSEL, WPARAM(sel_idx), LPARAM(0));
            }
        }
    }
    
    // Update scroll range
    let total_height = servers.len() as i32 * ROW_HEIGHT + SERVER_ITEM_MARGIN * 2;
    let mut rect = RECT::default();
    unsafe { GetClientRect(container, &mut rect).ok() };
    let visible_height = rect.bottom - rect.top;
    
    if total_height > visible_height {
        let si = SCROLLINFO {
            cbSize: std::mem::size_of::<SCROLLINFO>() as u32,
            fMask: SIF_RANGE | SIF_PAGE,
            nMin: 0,
            nMax: total_height,
            nPage: visible_height as u32,
            nPos: 0,
            nTrackPos: 0,
        };
        unsafe { 
            use windows::Win32::UI::Controls::SetScrollInfo;
            SetScrollInfo(container, SB_VERT, &si, true); 
        }
    }
}

// Resize server list items without rebuilding (for window resize performance)
#[cfg(windows)]
unsafe fn resize_server_list_items(container: HWND) {
    // Get container width
    let mut container_rect = RECT::default();
    unsafe { GetClientRect(container, &mut container_rect).ok() };
    let container_width = container_rect.right - container_rect.left;
    
    const LABEL_WIDTH: i32 = 130;
    const PORT_EDIT_WIDTH: i32 = 90;
    const COMBO_WIDTH: i32 = 130;
    const RIGHT_CONTROLS_WIDTH: i32 = LABEL_WIDTH + PORT_EDIT_WIDTH + COMBO_WIDTH + 30;
    let checkbox_width = container_width - RIGHT_CONTROLS_WIDTH - 20;
    let right_controls_x = 10 + checkbox_width + 10;
    
    // Get server count from global state
    if let Ok(global_servers) = VPN_SERVERS.lock() {
        if let Some(servers) = &*global_servers {
            for idx in 0..servers.len() {
                let y_pos = idx as i32 * ROW_HEIGHT + 10; // SERVER_ITEM_MARGIN = 10
                
                // Resize and reposition checkbox
                if let Ok(checkbox) = unsafe { GetDlgItem(container, ID_SERVER_CHECKBOX_BASE + idx as i32) } {
                    if !checkbox.is_invalid() {
                        unsafe {
                            SetWindowPos(
                                checkbox,
                                None,
                                10,
                                y_pos,
                                checkbox_width,
                                CONTROL_HEIGHT,
                                SWP_NOZORDER,
                            ).ok();
                        }
                    }
                }
                
                // Reposition label
                if let Ok(label) = unsafe { GetDlgItem(container, ID_SERVER_LABEL_BASE + idx as i32) } {
                    if !label.is_invalid() {
                        unsafe {
                            SetWindowPos(
                                label,
                                None,
                                right_controls_x,
                                y_pos + 5,
                                LABEL_WIDTH,
                                CONTROL_HEIGHT,
                                SWP_NOZORDER,
                            ).ok();
                        }
                    }
                }
                
                // Reposition port edit
                if let Ok(port_edit) = unsafe { GetDlgItem(container, ID_SERVER_PORT_EDIT_BASE + idx as i32) } {
                    if !port_edit.is_invalid() {
                        unsafe {
                            SetWindowPos(
                                port_edit,
                                None,
                                right_controls_x + LABEL_WIDTH + 5,
                                y_pos,
                                PORT_EDIT_WIDTH,
                                CONTROL_HEIGHT,
                                SWP_NOZORDER,
                            ).ok();
                        }
                    }
                }
                
                // Reposition combo
                if let Ok(combo) = unsafe { GetDlgItem(container, ID_SERVER_PROXY_COMBO_BASE + idx as i32) } {
                    if !combo.is_invalid() {
                        unsafe {
                            SetWindowPos(
                                combo,
                                None,
                                right_controls_x + LABEL_WIDTH + 5 + PORT_EDIT_WIDTH + 10,
                                y_pos,
                                COMBO_WIDTH,
                                CONTROL_HEIGHT,
                                SWP_NOZORDER,
                            ).ok();
                        }
                    }
                }
            }
        }
    }
}

// Helper function to destroy child windows
#[cfg(windows)]
unsafe extern "system" fn destroy_child_window(hwnd: HWND, _: LPARAM) -> BOOL {
    unsafe { DestroyWindow(hwnd).ok() };
    true.into()
}
