# AoE2 DE LAN Launcher

Offline LAN launcher for Age of Empires II: Definitive Edition using the [ageLANServer](https://github.com/luskaner/ageLANServer) backend.

## Overview

A standalone Windows executable that configures and launches Age of Empires II: Definitive Edition for local network and virtual LAN (Radmin VPN) multiplayer.

- **Single portable file**: All runtime dependencies (`launcher`, `server`, and `battle-server-manager`) are embedded directly into the binary and extracted automatically on first run.
- **Network auto-detection**: Detects available IPv4 network adapters and prioritizes Radmin VPN interfaces (`26.x.x.x`).
- **Player configuration**: Automatically writes Goldberg Steam emulator files (`steam_settings`, `configs.user.ini`, persona name, and random Steam ID).
- **Civilization themes**: Selectable civilization backgrounds and musical themes with volume controls.
- **Clean exit**: Reverts system hosts redirects via `config.exe` when the launcher closes.

## Requirements

- Windows 10 or 11 (64-bit)
- Age of Empires II: Definitive Edition
- Microsoft Edge WebView2 (included with Windows 10/11)

## Usage

1. Download `AoE2_LAN_Launcher.exe` from the [Releases](https://github.com/pecora31/age2delauncher/releases) page.
2. Run `AoE2_LAN_Launcher.exe`.
3. Set your **Game Directory** to the folder containing `AoE2DE_s.exe`.
4. Choose your role:
   - **Host**: Starts the battle server manager and hosts the match.
   - **Client**: Connects to the host using their local or Radmin IP address.
5. Set your player name and Steam ID (or keep the generated defaults).
6. Click **Start Game**.

## Building from Source

### Prerequisites

- [Node.js](https://nodejs.org/) (v18 or newer)
- [Rust](https://rustup.rs/) (stable toolchain)

### Build Instructions

```powershell
# Clone the repository
git clone https://github.com/pecora31/age2delauncher.git
cd age2delauncher

# Install frontend dependencies
npm install

# Build release executable
npm run tauri build
```

The resulting standalone executable will be located at:
`src-tauri/target/release/AoE2_LAN_Launcher.exe`

## Credits

- [ageLANServer](https://github.com/luskaner/ageLANServer) by Luskaner
- Goldberg Steam Emulator
- Built with [Tauri v2](https://tauri.app/)

## License

MIT
