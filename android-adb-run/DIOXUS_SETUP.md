# Dioxus GUI Setup Guide

## Prerequisites for Linux

To enable the Dioxus desktop GUI, you need to install GTK development libraries:

```bash
# On Ubuntu/Debian:
sudo apt update
sudo apt install -y libgtk-3-dev libglib2.0-dev libwebkit2gtk-4.1-dev pkg-config

# On Fedora/RHEL:
sudo dnf install gtk3-devel glib2-devel webkit2gtk4.1-devel pkgconfig

# On Arch Linux:
sudo pacman -S gtk3 glib2 webkit2gtk-4.1 pkgconf
```

## Enable GUI

1. Uncomment the dioxus dependency in `Cargo.toml`:
```toml
[dependencies]
dioxus = { version = "0.6.3", features = ["desktop"] }
serde = { version = "1.0", features = ["derive"] }
```

2. Uncomment the GUI binary in `Cargo.toml`:
```toml
[[bin]]
name = "android-adb-run-gui"
path = "src/gui.rs"
```

3. Uncomment the dioxus module in `src/lib.rs`:
```rust
pub mod adb;
pub mod dioxus;
```

4. Build and run the GUI:
```bash
cargo run --bin android-adb-run-gui
```

## GUI Features

The Dioxus GUI provides:
- Device information display (name, transport ID, screen size)
- Take screenshot button
- Exit button
- Clean, modern interface

The GUI uses the same ADB automation library as the command-line tool.
