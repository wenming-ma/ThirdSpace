# ThirdSpace

A Windows system tray translation utility powered by AI models via OpenRouter.

## Features

- **Global Hotkey**: Press `Ctrl+Alt+T` to instantly translate clipboard text
- **System Tray**: Runs quietly in background with quick access menu
- **AI-Powered**: Uses OpenRouter API to access multiple AI models
- **Toast Notifications**: Shows translation results as system notifications
- **Configurable**: Choose your preferred AI model and target language

## Installation

### Via WinGet (Coming Soon)

```bash
winget install wenming.ThirdSpace
```

### Manual Installation

Download the latest `.msi` installer from [Releases](https://github.com/wenming-ma/ThirdSpace/releases).

## Usage

1. Copy any text to clipboard
2. Press `Ctrl+Alt+T` (or use the tray menu)
3. Translation appears as a toast notification and is copied to clipboard

## Configuration

Right-click the system tray icon and select **Settings** to configure:

- **OpenRouter API Key**: Get one at [openrouter.ai](https://openrouter.ai)
- **Target Language**: Language to translate into (default: English)
- **AI Model**: Select from available OpenRouter models

## Requirements

- Windows 10/11
- OpenRouter API key

## Built With

- [Tauri v2](https://tauri.app/) - Native app framework
- [Rust](https://www.rust-lang.org/) - Backend
- [OpenRouter](https://openrouter.ai/) - AI model API

## License

MIT
