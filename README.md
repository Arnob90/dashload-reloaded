# ⚡ Dashload Reloaded

**A blazing-fast, lightweight, open-source download manager built with Rust, Tauri v2, and React.**

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Built with Rust](https://img.shields.io/badge/Backend-Rust%20%2B%20Tokio-orange?logo=rust)](https://www.rust-lang.org/)
[![Tauri v2](https://img.shields.io/badge/Framework-Tauri%20v2-24C8DB?logo=tauri)](https://v2.tauri.app/)
[![React + Mantine](https://img.shields.io/badge/Frontend-React%20%2B%20Mantine%20v7-339af0?logo=react)](https://mantine.dev/)
[![PRs Welcome](https://img.shields.io/badge/PRs-welcome-brightgreen.svg)](CONTRIBUTING.md)

## 🚀 Why Dashload?

Most modern download managers are either **closed-source and paid** (like IDM), **bundle adware and telemetry** (like FDM), or are **bloated Electron applications** that consume 300+ MB of RAM just sitting idle in the system tray (like Motrix).

**Dashload** is engineered in **Rust** to deliver maximum multi-threaded download throughput directly to disk with near-zero memory footprint and no temporary file-merging delays.

---

## 📊 Benchmarks & Comparison

| Feature | IDM | FDM | Motrix (Electron) | **Dashload Reloaded** |
| :--- | :---: | :---: | :---: | :---: |
| **Engine Core** | C++ (Win32) | Closed C++ | C++ (`aria2`) | **Rust (Tokio Async I/O)** |
| **Idle Memory (RAM)** | ~20 MB | ~150 MB | ~280 MB | **~25 MB** |
| **License** | Paid ($25/yr) | Closed Source | MIT | **100% Free & Open Source (MIT)** |
| **Platform Support** | Windows only | Win / Mac / Linux | Win / Mac / Linux | **Windows / macOS / Linux** |
| **Disk Write Model** | Temp files + Merge | Temp files + Merge | Pre-allocated | **Direct-to-Disk Seeking (0-Merge)** |
| **Adware / Telemetry** | ❌ None | ⚠️ Bundled promos | ❌ None | **❌ 100% Clean / Zero Telemetry** |

---

## ✨ Key Features

- **🛡️ Auto-Healing Integrity & Resume:** Never corrupts sparse files. If a partial download is moved or deleted externally while paused, Dashload detects it and automatically prevents file corruption.
- **🔒 Integrated Streaming Checksum Verification:** Cryptographically validates SHA-256, SHA-512, or MD5 hashes via low-footprint 8 KB streaming buffers offloaded to background threads.
- **🌐 Browser Extension Integration:** Native bridge on `localhost:3000` intercepts browser downloads, automatically forwarding authentication cookies, referrers, and user-agents so login-protected downloads work out of the box.
- **📊 Real-time Lock-Free UI:** Powered by TanStack Table, React Query v5, and Mantine v7. Updates speeds and progress smoothly with zero DOM thrashing.

---

## 🛠️ Architecture

Dashload separates concerns strictly across an asynchronous systems-level core and a reactive desktop presentation layer:

```text
┌────────────────────────────────────────────────────────┐
│               React + Mantine v7 Frontend              │
│       TanStack Table v8  •  TanStack Query v5          │
└───────────────────────────▲────────────────────────────┘
                            │ Tauri v2 IPC Bridge
┌───────────────────────────▼────────────────────────────┐
│                  Tauri v2 Rust Runtime                 │
│    Lock-Free Status Readouts  •  Local Extension Pipe  │
└───────────────────────────▲────────────────────────────┘
                            │
┌───────────────────────────▼────────────────────────────┐
│                 Dashload Engine (Rust)                 │
│  - Tokio Multi-threaded Async Runtime                  │
│  - InPlaceSegment Worker Pipeline                      │
│  - Streaming SHA/MD5 Verifier in spawn_blocking        │
│  - Atomic JSON Session Persistence                     │
└────────────────────────────────────────────────────────┘
```

---

## 📥 Installation

Pre-compiled binary releases for all major platforms can be downloaded directly from the **[Releases](../../releases)** tab:

- **Windows:** `.msi` / `.exe`
- **Linux:** `.deb` / `.AppImage`
- **macOS:** `.dmg` (Universal Apple Silicon & Intel)

---

## 🔨 Building from Source

### Prerequisites

Ensure you have the following installed on your machine:

- [Rust](https://www.rust-lang.org/)
- [Node.js](https://nodejs.org/)
- [Bun](https://bun.com/)

On Linux, install required webview dependencies:

```bash
# Ubuntu / Debian
sudo apt update && sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev

# Arch Linux / Manjaro
sudo pacman -S webkit2gtk-4.1 base-devel curl wget file openssl appmenu-gtk-module libappindicator-gtk3 librsvg
```

### Steps

1. **Clone the repository:**

   ```bash
   git clone https://github.com/Arnob90/dashload-reloaded.git
   cd dashload-reloaded
   ```

2. **Install frontend dependencies:**

   ```bash
   npm install
   ```

3. **Run in development mode:**

   ```bash
   npm run tauri dev
   ```

4. **Build release installer:**

   ```bash
   npm run tauri build
   ```

---

## 🛣️ Roadmap

- [x] Multi-connection accelerated HTTP/HTTPS engine
- [x] Direct-to-file seeking with zero merge overhead
- [x] Auto-healing resume and pause lifecycle
- [x] Background stream checksum verifier (SHA256, SHA512, MD5)
- [x] Browser extension bridge with cookie & header forwarding
- [x] System tray minimization and background daemon mode
- [ ] Global bandwidth limiter & speed scheduler
- [ ] Max simultaneous download queue manager
- [ ] BitTorrent engine integration (via `librqbit`)
- [ ] HLS / `.m3u8` video stream sniffer and assembler

---

## 🤝 Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

1. Fork the Project
2. Create your Feature Branch (`git checkout -b feature/AmazingFeature`)
3. Commit your Changes (`git commit -m 'Add some AmazingFeature'`)
4. Push to the Branch (`git push origin feature/AmazingFeature`)
5. Open a Pull Request

---

## 📄 License

Distributed under the **MIT License**. See `LICENSE` for more information.
