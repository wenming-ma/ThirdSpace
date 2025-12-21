# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

ThirdSpace is a Windows system tray translation utility that uses AI models via OpenRouter to translate clipboard text. It features a global hotkey (Ctrl+Alt+T), a settings UI, and toast notifications.

Built with **Tauri v2** for native performance and proper transparent window support.

## Key Commands

### Build and Run
```bash
cd src-tauri
cargo build              # Build the project
cargo run                # Run the application
cargo build --release    # Build optimized release binary
```

### Using Tauri CLI (recommended)
```bash
npx @tauri-apps/cli dev      # Development mode with hot reload
npx @tauri-apps/cli build    # Production build with installer
```

### Checking
```bash
cd src-tauri
cargo check          # Quick compilation check
cargo clippy         # Linter for catching common mistakes
```

## Architecture

### Project Structure
```
ThirdSpace/
├── src-tauri/           # Tauri v2 backend (Rust)
│   ├── Cargo.toml
│   ├── tauri.conf.json  # App config, window settings
│   ├── capabilities/    # Permission definitions
│   ├── icons/           # App icons (ICO, ICNS, PNG)
│   └── src/
│       ├── main.rs      # Entry point
│       ├── lib.rs       # Commands, state, tray, hotkey
│       ├── config.rs    # Config persistence
│       ├── openrouter.rs # API client
│       └── prompt.rs    # Translation prompts
├── ui/                  # Frontend (HTML/CSS/JS)
│   ├── settings.html    # Settings window
│   └── toast.html       # Toast notification
└── assets/              # Source icons
```

### Tauri v2 Architecture
- **Frontend**: HTML/CSS/JS in `ui/` folder, communicates via `invoke()`
- **Backend**: Rust in `src-tauri/src/`, exposes Tauri commands
- **IPC**: Frontend calls `invoke('command_name', { args })` to execute Rust functions
- **Events**: Backend emits events to frontend via `emit()`, frontend listens with `listen()`

### Tauri Plugins Used
- `tauri-plugin-clipboard-manager` - Clipboard read/write
- `tauri-plugin-global-shortcut` - Global hotkey registration
- `tauri-plugin-shell` - Open external links

### Core Flow
1. User triggers translation (hotkey Ctrl+Alt+T or tray menu)
2. Clipboard text is read via clipboard plugin
3. Translation prompt is built using markers (`<<<TRANSLATION>>>` / `<<<END_TRANSLATION>>>`)
4. OpenRouter API is called asynchronously
5. Response is parsed and extracted content between markers
6. Translated text is written back to clipboard
7. Toast notification displays success/error

### Module Responsibilities
- **lib.rs**: App setup, Tauri commands, system tray, global shortcut handler
- **openrouter.rs**: API client for OpenRouter chat completions
- **prompt.rs**: Builds structured prompts with translation markers
- **config.rs**: Loads/saves JSON config via `dirs` crate

### Tauri Commands
```rust
#[tauri::command]
fn get_config(state: State<AppState>) -> Config

#[tauri::command]
async fn save_config(app: AppHandle, state: State<AppState>, new_config: Config) -> Result<(), String>

#[tauri::command]
async fn translate(app: AppHandle, state: State<AppState>) -> Result<(), String>
```

### Window Configuration
- **Toast window**: Transparent (`shadow: false`), always on top, 200x56px pill shape
- **Settings window**: Decorated, 480x520px, centered

### Configuration
Default model: `google/gemini-2.5-flash-preview-05-20`
Default target language: `English`
Reasoning is enabled by default (`reasoning_enabled: true`)
Config persists to `%APPDATA%/ThirdSpace/config.json`

### Translation Protocol
The prompt engineering uses explicit markers to extract clean translations:
- Input wraps text with `%%` separators for multi-paragraph content
- Output must be wrapped in `<<<TRANSLATION>>>` ... `<<<END_TRANSLATION>>>`
- `prompt::extract_translation()` parses the marked section from LLM response
