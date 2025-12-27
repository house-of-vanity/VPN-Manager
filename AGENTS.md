# AGENTS.md - VPN Tray Manager Development Guide

## Project Overview

**Xray-VPN-Manager** is a Windows-only system tray application for managing multiple Xray-core VPN servers from a single subscription URL. Built in Rust with native Windows UI (no web/electron), it provides a lightweight way to enable/disable VPN servers with custom proxy ports.

**Platform:** Windows 10/11 only (uses Windows API extensively)  
**Language:** Rust (edition 2024)  
**Architecture:** Native Win32 GUI + Tokio async runtime for process management

---

## Essential Commands

### Build & Run

```bash
# Development build
cargo build

# Release build (optimized)
cargo build --release

# Run (shows console output - #![windows_subsystem = "windows"] commented out in main.rs:1)
cargo run

# Release run (production mode)
cargo run --release
```

### Testing

No test suite currently exists. Manual testing required:
1. Run app, check tray icon appears
2. Open Settings, enter subscription URL and xray binary path
3. Click Update to fetch servers
4. Enable servers, set ports/proxy types
5. Click Save and verify servers start
6. Check tray menu shows running servers

### Linting & Formatting

```bash
# Check code (standard Rust linting)
cargo check

# Format code
cargo fmt

# Run Clippy for additional lints
cargo clippy
```

---

## Project Structure

```
VPN-Manager/
├── src/
│   ├── main.rs              # Entry point, tray icon, Windows message loop
│   ├── config.rs            # Config persistence (JSON in %APPDATA%)
│   ├── xray_manager.rs      # Xray process lifecycle management
│   ├── vpn/
│   │   └── mod.rs           # Subscription parsing, URI handling
│   └── ui/
│       ├── mod.rs           # UI module exports
│       ├── tray.rs          # Tray icon creation, menu rendering
│       └── settings_window.rs # Native Win32 settings window (1200+ LOC)
├── Cargo.toml               # Dependencies, Windows features
├── build.rs                 # Embeds app.manifest via app.rc
├── app.manifest             # Windows DPI awareness, compatibility
└── app.rc                   # Resource compiler input
```

### Module Responsibilities

- **main.rs**: Global state (`TOKIO_RUNTIME`, `MENU_UPDATE_REQUESTED`), server restart logic, Windows message pump
- **config.rs**: `Config` struct, load/save to `%APPDATA%\Xray-VPN-Manager\config.json`
- **xray_manager.rs**: Wraps `v2parser::xray_runner::XrayRunner`, manages server processes in `XRAY_PROCESSES` HashMap
- **vpn/mod.rs**: Fetches subscription URLs (base64-encoded), parses URIs (vless, vmess, trojan, ss, socks), assigns local ports
- **ui/tray.rs**: Creates tray icon (gold star), builds dynamic menu with running servers
- **ui/settings_window.rs**: Complex native Win32 window with custom scrolling, file dialogs, dynamic server list

---

## Configuration & Data Flow

### Config File

**Location:** `%APPDATA%\Xray-VPN-Manager\config.json`

```json
{
  "subscription_url": "https://example.com/sub",
  "xray_binary_path": "C:\\path\\to\\xray.exe",
  "server_settings": {
    "VLESS://server1.com:443": {
      "local_port": 1080,
      "proxy_type": "SOCKS",
      "enabled": true
    }
  }
}
```

**Server Key Format:** `PROTOCOL://address:port` (e.g., `VLESS://server.com:443`)

### Data Flow

1. **Startup:** 
   - Load config from `%APPDATA%`
   - Fetch subscription URIs
   - Parse and assign ports (preserve saved settings)
   - Store in `VPN_SERVERS` global mutex
   - Start enabled servers via `xray_manager::start_server()`

2. **Settings Window:**
   - User enters/updates subscription URL
   - Click "Update" → fetch and parse in background thread
   - Store in `VPN_SERVERS`, post `WM_UPDATE_SERVERS` message
   - Rebuild server list UI with checkboxes, port edits, proxy type combos
   - Click "Save" → build `Config` from UI state, save to JSON, call `restart_xray_servers()`

3. **Server Control:**
   - `restart_xray_servers()` stops all, starts enabled ones
   - Updates tray menu via `request_menu_update()` (sets atomic flag)
   - Main loop checks flag and calls `update_tray_menu()`

---

## Code Patterns & Conventions

### Global State

Uses `LazyLock` (nightly-stabilized) for lazy static initialization:

```rust
pub static TOKIO_RUNTIME: LazyLock<tokio::runtime::Runtime> = LazyLock::new(|| {
    tokio::runtime::Runtime::new().expect("Failed to create tokio runtime")
});

pub static VPN_SERVERS: Mutex<Option<Vec<VpnServer>>> = Mutex::new(None);
pub static XRAY_PROCESSES: LazyLock<Mutex<HashMap<String, XrayRunner>>> = ...;
pub static MENU_UPDATE_REQUESTED: AtomicBool = AtomicBool::new(false);
```

### Threading Model

- **Main thread:** Windows message loop (synchronous, blocking `GetMessageW`)
- **Background threads:** Subscription fetching, server start/stop (spawn via `std::thread::spawn`)
- **Async runtime:** Tokio runtime for xray process management, accessed via `TOKIO_RUNTIME.block_on(async { ... })`

### Error Handling

- Functions return `Result<T, String>` (error messages as strings)
- Silent failures common in networking code (vpn/mod.rs): `Err(_) => {}` without logging
- Print statements (`println!`, `eprintln!`) for debugging (no structured logging)

### Naming Conventions

- **Functions:** `snake_case` (Rust standard)
- **Types:** `PascalCase` (`VpnServer`, `ServerSettings`)
- **Constants:** `UPPER_SNAKE_CASE` (`ID_URL_EDIT`, `MARGIN`, `WM_UPDATE_SERVERS`)
- **Control IDs:** Sequential ranges (checkboxes: 2000+, port edits: 3000+, combos: 4000+, labels: 5000+)

### Windows API Patterns

Uses `windows` crate (v0.58) with extensive feature flags:

```rust
#[cfg(windows)]
use windows::{
    Win32::{
        Foundation::{HWND, LPARAM, WPARAM},
        UI::WindowsAndMessaging::*,
    },
};
```

- **String encoding:** UTF-16 conversion everywhere: `"text\0".encode_utf16().collect::<Vec<u16>>()`
- **Resource management:** No explicit cleanup (relies on OS to clean up on process exit)
- **Unsafe:** Almost all Win32 calls wrapped in `unsafe { }` blocks
- **Error handling:** `.ok()`, `.unwrap()`, `.expect()` common (crashes on critical failures)

### UI Control Layout

Constants define consistent spacing:

```rust
const MARGIN: i32 = 15;
const FONT_SIZE: i32 = 32;
const LABEL_HEIGHT: i32 = 45;
const CONTROL_HEIGHT: i32 = 45;
const ROW_HEIGHT: i32 = 55;
```

Dynamic widths calculated from window size in `WM_SIZE` handler.

---

## Dependencies & External Tools

### Key Rust Dependencies

- **tray-icon (0.21):** Cross-platform tray icon (uses Windows native API)
- **windows (0.58):** Direct Win32 API bindings (extensive feature list in Cargo.toml)
- **tokio (1.x):** Async runtime for process management (rt-multi-thread, sync, macros)
- **serde/serde_json (1.x):** Config serialization
- **reqwest (0.12):** HTTP client for subscription fetching (blocking feature)
- **base64 (0.22):** Decode subscription content
- **image (0.25):** Image handling (unused in current code?)
- **v2parser (local path):** Custom parser for VPN URIs (path: `../v2-uri-parser`)

### Critical External Dependency

**v2parser** is a local crate located at `../v2-uri-parser` (relative to project root). Must exist for project to build.

Provides:
- `v2parser::parser::get_metadata(uri: &str) -> String` (JSON string)
- `v2parser::parser::create_json_config(uri, socks_port, http_port) -> String`
- `v2parser::xray_runner::XrayRunner` (process management)

### External Binaries

**xray-core:** User must download separately from https://github.com/XTLS/Xray-core/releases  
Binary path configured in settings window and saved to config.

---

## Architecture Details

### Async Runtime Usage

Tokio runtime is created once at startup and used for:
- Starting xray processes: `TOKIO_RUNTIME.block_on(async { xray_manager::start_server(...).await })`
- Stopping processes: `TOKIO_RUNTIME.block_on(async { xray_manager::stop_all_servers().await })`

**Important:** Main thread blocks on async operations (no true concurrency for process management).

### Process Management

Each enabled server spawns an xray process via `v2parser::xray_runner::XrayRunner`:
- Stored in global `XRAY_PROCESSES: HashMap<server_key, XrayRunner>`
- Server key format: `PROTOCOL://address:port`
- Processes cleaned up on `stop_server()` or `stop_all_servers()`
- All processes stopped on app exit (main.rs:154)

### Tray Menu Updates

Two-step update mechanism:
1. Call `request_menu_update()` to set `MENU_UPDATE_REQUESTED` atomic flag
2. Main loop checks flag and calls `update_tray_menu(tray_icon, settings_item, quit_item)`
3. Creates new menu with `create_tray_menu_with_servers()` and sets it via `tray_icon.set_menu()`

Why? `tray-icon` doesn't support callbacks, and menu must be updated from main thread.

### Settings Window Lifecycle

- Created on "Settings" menu item click
- HWND stored in `Arc<Mutex<Option<HWND>>>` to prevent duplicates
- Brings to front if already open (main.rs:140-151)
- Custom window class "SettingsWindowClass" with white background
- Custom scroll container class "ScrollContainerClass" with manual scroll handling

---

## Common Gotchas

### 1. Windows-Only Build

Project **will not compile** on Linux/macOS due to:
- `#[cfg(windows)]` everywhere
- Direct Win32 API calls
- Platform-specific dependencies

### 2. Edition 2024 Requirement

`Cargo.toml` specifies `edition = "2024"` (unreleased as of knowledge cutoff). May need Rust nightly or change to `edition = "2021"`.

### 3. Local Dependency Path

`v2parser = { path = "../v2-uri-parser" }` must exist at that exact relative path. Not published on crates.io.

### 4. Windows Subsystem Flag

`main.rs:1` has `#![windows_subsystem = "windows"]` commented out for debugging. In release:
- Uncomment to hide console window
- Comment to see stdout/stderr

### 5. Manifest Embedding

`build.rs` embeds `app.manifest` via `embed-resource` crate. Required for:
- DPI awareness (PerMonitorV2)
- Windows 10/11 compatibility flags
- Non-admin execution (asInvoker)

### 6. UTF-16 Null Termination

All Windows API strings **must** be null-terminated:
```rust
let text: Vec<u16> = "Hello\0".encode_utf16().collect();  // ✅ Correct
let text: Vec<u16> = "Hello".encode_utf16().collect();    // ❌ Crashes or garbage
```

### 7. Control IDs Must Be Unique

Each window control needs unique ID for `GetDlgItem()`:
- Checkboxes: `ID_SERVER_CHECKBOX_BASE + index` (2000+)
- Port edits: `ID_SERVER_PORT_EDIT_BASE + index` (3000+)
- Combos: `ID_SERVER_PROXY_COMBO_BASE + index` (4000+)

ID collision causes controls to be unreachable.

### 8. Custom Message Range

Custom Windows messages start at `WM_USER + 1`:
```rust
const WM_UPDATE_SERVERS: u32 = WM_USER + 1;
```

Don't use values below `WM_USER` (conflicts with system messages).

### 9. Background Thread UI Updates

Cannot update UI from background threads. Use `PostMessageW()` to marshal to main thread:
```rust
std::thread::spawn(move || {
    let servers = fetch_and_process_vpn_list(&url);
    unsafe {
        PostMessageW(hwnd, WM_UPDATE_SERVERS, WPARAM(0), LPARAM(0));
    }
});
```

### 10. Scroll Container Children

Server list controls are children of scroll container, not main window. Must use:
```rust
GetDlgItem(container, control_id)  // ✅ Correct
GetDlgItem(hwnd, control_id)       // ❌ Returns error
```

---

## Testing Approach

### Manual Testing Checklist

1. **Tray Icon:**
   - [ ] Icon appears in system tray (gold star)
   - [ ] Right-click shows menu (Settings, Exit)
   - [ ] Exit cleanly stops all processes

2. **Settings Window:**
   - [ ] Opens on Settings click
   - [ ] Focuses if already open (no duplicates)
   - [ ] URL and Xray path persist from config
   - [ ] Browse button opens file dialog
   - [ ] Update fetches servers and populates list

3. **Server List:**
   - [ ] Checkboxes toggle enabled state
   - [ ] Port edits accept numbers only
   - [ ] Proxy type combo shows SOCKS/HTTP
   - [ ] Scroll works with mouse wheel and scrollbar
   - [ ] Window resize adjusts layout

4. **Save Functionality:**
   - [ ] Save writes to `%APPDATA%\Xray-VPN-Manager\config.json`
   - [ ] Servers restart on save
   - [ ] Tray menu updates with running servers
   - [ ] Settings persist after app restart

5. **Error Handling:**
   - [ ] Invalid URL shows no error (silent failure)
   - [ ] Missing xray binary path causes start failures (check console)
   - [ ] Invalid port numbers handled gracefully

### No Automated Tests

Currently no unit tests, integration tests, or CI/CD pipeline. All validation is manual.

---

## Adding New Features

### Adding a New Menu Item

1. Create `MenuItem` in `main.rs`:
   ```rust
   let new_item = MenuItem::new("New Feature", true, None);
   ```

2. Add to menu in `ui/tray.rs`:
   ```rust
   tray_menu.append(&new_item).unwrap();
   ```

3. Handle event in main loop:
   ```rust
   if event.id == new_item.id() {
       // Handle click
   }
   ```

### Adding a Config Field

1. Add to `Config` struct in `config.rs`:
   ```rust
   pub struct Config {
       pub subscription_url: String,
       pub xray_binary_path: String,
       pub new_field: String,  // Add this
       #[serde(default)]
       pub server_settings: HashMap<String, ServerSettings>,
   }
   ```

2. Update `Default` impl:
   ```rust
   fn default() -> Self {
       Config {
           // ...
           new_field: String::new(),
           // ...
       }
   }
   ```

3. Add UI control in `settings_window.rs`:
   - Define control ID constant
   - Create control in `create_controls()`
   - Read value in Save button handler

### Adding a New VPN Protocol

1. Update `parse_vpn_uri()` in `vpn/mod.rs`:
   ```rust
   let is_supported = uri.starts_with("vless://") 
       || uri.starts_with("newprotocol://");  // Add this
   ```

2. Ensure `v2parser` crate supports the protocol (external dependency).

---

## Debugging Tips

### Enable Console Output

Ensure `#![windows_subsystem = "windows"]` is **commented** in `main.rs:1`.

### Common Debug Points

1. **Subscription fetch fails:**
   - Check URL is valid and returns base64-encoded content
   - Add `println!` in `fetch_and_process_vpn_list()` to see response

2. **Servers don't start:**
   - Verify xray binary path exists
   - Check `xray_manager::start_server()` errors in console
   - Ensure `v2parser` generates valid config

3. **Menu doesn't update:**
   - Verify `request_menu_update()` called
   - Check `MENU_UPDATE_REQUESTED` flag in main loop
   - Ensure `VPN_SERVERS` mutex populated

4. **Settings window controls missing:**
   - Check control IDs are unique
   - Verify `GetDlgItem()` uses correct parent (container vs. hwnd)
   - Look for "Failed to create..." messages

5. **DPI scaling issues:**
   - Verify `app.manifest` embedded (check `build.rs` runs)
   - Ensure `SetProcessDpiAwarenessContext` called at startup

### Useful Print Statements

```rust
println!("Settings window created: {:?}", hwnd);
println!("URL entered: {}", url);
println!("Started server: {}", server.name);
eprintln!("Failed to start server {}: {}", server.name, e);
```

Already present in code for basic debugging.

---

## Performance Considerations

### UI Thread Blocking

Main thread blocks on:
- `GetMessageW()` (Windows message pump)
- `TOKIO_RUNTIME.block_on()` (async operations)

Keep async operations fast to avoid UI freezes.

### Scroll Performance

Settings window with many servers (100+) may lag:
- Each server creates 4-5 controls (checkbox, label, edit, combo)
- Scroll handler moves all children on every scroll event
- Consider virtualization if server count exceeds ~50

### Subscription Fetch

Blocking HTTP call in background thread:
```rust
reqwest::blocking::get(url)
```

No timeout configured. May hang on slow/unresponsive servers.

---

## Security & Safety

### Unsafe Code

Extensive use of `unsafe` for Windows API:
- String pointer conversions (`PCWSTR::from_raw()`)
- Window handles (`HWND` casting)
- Message parameter packing (`WPARAM`, `LPARAM`)

**Assumption:** Windows API contracts upheld (e.g., null-terminated strings).

### Process Execution

Executes arbitrary xray binary from user-configured path. No validation of binary integrity.

### Network Requests

Fetches subscription URLs without TLS verification control. Uses `reqwest` defaults (should verify certificates).

### Config Storage

Plaintext JSON in `%APPDATA%`. No encryption. Contains:
- Subscription URLs (may include credentials in query params)
- Server addresses/ports
- Local proxy ports

**Not suitable for highly sensitive credentials.**

---

## Future Improvements

Based on code analysis:

1. **Add tests:** Unit tests for config, integration tests for subscription parsing
2. **Logging:** Replace `println!` with structured logging (e.g., `tracing`, `env_logger`)
3. **Error handling:** Return structured errors instead of `String`, display errors in UI
4. **Timeouts:** Add request timeouts to `reqwest::blocking::get()`
5. **Virtualized list:** For 50+ servers, implement virtual scrolling
6. **Tray menu callbacks:** Investigate better tray update mechanism (polling is suboptimal)
7. **CI/CD:** Add GitHub Actions for Windows builds
8. **Installer:** Package as MSI/NSIS installer instead of bare `.exe`
9. **Auto-update:** Check for new versions on startup
10. **Connection testing:** Ping/test servers before enabling

---

## Build Troubleshooting

### "edition 2024 not found"

Change `Cargo.toml`:
```toml
edition = "2021"  # Change from 2024
```

### "v2parser not found"

Ensure `../v2-uri-parser` exists relative to project root. Clone or create it separately.

### "embed-resource failed"

Ensure Windows SDK installed (required for `rc.exe` resource compiler).

### Missing Windows features

If compile fails with missing types, add to `Cargo.toml`:
```toml
[target.'cfg(windows)'.dependencies]
windows = { version = "0.58", features = [
    # Add missing feature here
] }
```

---

## Summary for AI Agents

When working in this codebase:

1. **Platform:** Windows-only, will not compile elsewhere
2. **External dep:** `v2parser` at `../v2-uri-parser` required
3. **Testing:** Manual only, no automated tests
4. **UI:** Native Win32, complex custom controls in `settings_window.rs`
5. **Async:** Tokio runtime used, but main thread blocks on operations
6. **State:** Global mutexes (`VPN_SERVERS`, `XRAY_PROCESSES`) shared across threads
7. **Config:** `%APPDATA%\Xray-VPN-Manager\config.json`, plaintext JSON
8. **Strings:** Always UTF-16 with null terminator for Windows API
9. **IDs:** Control IDs must be unique, use defined constants + index
10. **Updates:** UI updates from background threads via `PostMessageW()`

**Most complex file:** `ui/settings_window.rs` (1200+ lines, custom scrolling, dynamic layout)  
**Most critical function:** `restart_xray_servers()` in `main.rs` (stops/starts all servers)  
**Most fragile part:** Windows API unsafe code (crashes if assumptions violated)
