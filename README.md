# Rabbitty

<div align="center">
  <img src="assets/logo.png" alt="Rabbitty logo" width="200" />
  <p>Fast, lean, cross-platform terminal emulator.</p>
</div>

> Warn: This is a work-in-progress project.

Rabbitty is a terminal emulator chasing `foot`-like memory thrift and cross-platform speed, with feature-ful and polish.

- Lean memory: small, steady footprint even with deep scrollback.
- Fast paths: low-latency rendering and input.
- Cross-platform: consistent on macOS, Linux, Windows.
- Featureful and fancy: tabs, themes, and modern UX without bloat.

## Install

**Linux / macOS:**
```sh
curl -fsSL https://raw.githubusercontent.com/wHoIsDReAmer/RabbiTTY/main/install.sh | sh
```

**Windows (PowerShell):**
```powershell
irm https://raw.githubusercontent.com/wHoIsDReAmer/RabbiTTY/main/install.ps1 | iex
```

**From source (`cargo install`):**
```sh
cargo install --git https://github.com/wHoIsDReAmer/RabbiTTY
```

The Unix script installs the binary to `~/.local/bin/rabbitty`. On Linux it also installs a desktop launcher and icon under `~/.local/share` so Rabbitty appears in GNOME, KDE, and other XDG-compatible app launchers. On macOS it installs the `.app` bundle to `~/Applications`. The PowerShell script installs to `%LOCALAPPDATA%\Rabbitty` and adds it to your user PATH.

## Goals

- [x] SSH Managing
- [ ] Plugin support with wasm
- [x] Easy changing theme
- [x] i18n (English, 한국어)
- [ ] Easy file upload & download with SFTP
- [ ] Split terminal in single tab

## Custom Themes

Rabbitty ships with built-in color schemes (Catppuccin Mocha, Dracula, Tokyo Night, Nord, One Dark, Gruvbox Dark, Solarized Dark) and supports user-defined themes via TOML files.

### Adding a custom theme

1. Create the themes directory:
   ```
   mkdir -p ~/.config/rabbitty/themes
   ```
2. Add a `.toml` file (e.g. `~/.config/rabbitty/themes/my-theme.toml`):
   ```toml
   name = "My Custom Theme"
   foreground = "#c0caf5"
   background = "#1a1b26"
   cursor = "#c0caf5"

   [ansi]
   black = "#15161e"
   red = "#f7768e"
   green = "#9ece6a"
   yellow = "#e0af68"
   blue = "#7aa2f7"
   magenta = "#bb9af7"
   cyan = "#7dcfff"
   white = "#a9b1d6"
   bright_black = "#414868"
   bright_red = "#f7768e"
   bright_green = "#9ece6a"
   bright_yellow = "#e0af68"
   bright_blue = "#7aa2f7"
   bright_magenta = "#bb9af7"
   bright_cyan = "#7dcfff"
   bright_white = "#c0caf5"
   ```
3. Restart Rabbitty — the theme appears in **Settings > Theme > Color Scheme**.

A theme with the same name as a built-in will override it. See `assets/example-theme.toml` for a full reference.

## Supported Platforms

- Linux (x86_64, aarch64)
- Windows (x86_64)
- macOS (Apple Silicon)
