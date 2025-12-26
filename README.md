# VPN Tray Manager

Simple Windows system tray app for managing xray-core VPN servers.

## Features

- System tray icon with context menu
- Auto-start enabled VPN servers on launch
- Manage multiple servers from subscription URL
- Configure local ports and proxy types (SOCKS/HTTP)
- Enable/disable servers individually
- Servers restart automatically after config changes

## Requirements

- Windows 10/11
- [xray-core](https://github.com/XTLS/Xray-core/releases) binary

## Usage

1. Run `win-test-tray.exe`
2. Right-click tray icon → Settings
3. Enter subscription URL and xray binary path
4. Check servers you want to enable
5. Click Save

Running servers appear in the tray menu:
```
✓ Server Name (SOCKS:1080)
✓ Another Server (HTTP:1081)
---
Settings
---
Exit
```

## Config Location

`%APPDATA%\win-test-tray\config.json`

## Build

```bash
cargo build --release
```
