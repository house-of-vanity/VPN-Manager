# VPN Tray Manager

Simple Windows system tray app for managing all xray-core VPN servers using just one SUB link.

## Features

- System tray icon
- Manage multiple servers from subscription URL
- Enable/disable servers individually

## Requirements

- Windows 10/11
- [xray-core](https://github.com/XTLS/Xray-core/releases) binary

## Usage

1. Run `Xray-VPN-Manager.exe`
2. Right-click tray icon → Settings
3. Enter subscription URL and xray binary path
4. Check servers you want to enable
5. Click Save

Running servers appear in the tray menu:
```
✓ USA (SOCKS:1080)
✓ France (HTTP:1081)
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
