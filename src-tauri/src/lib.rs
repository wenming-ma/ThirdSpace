mod config;
mod openrouter;
mod prompt;

use config::Config;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, SystemTime};
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder,
};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};
use tracing::{debug, error, info, Instrument};

const TOAST_DURATION_MS: u64 = 2200;
const LOG_RETENTION_DAYS: u64 = 14;
const LOG_FILE_PREFIX: &str = "thirdspace.log";
const DEFAULT_LOG_FILTER: &str = "info,tauri=warn,reqwest=warn,hyper=warn";

static REQUEST_SEQ: AtomicU64 = AtomicU64::new(1);

fn next_request_id() -> u64 {
    REQUEST_SEQ.fetch_add(1, Ordering::Relaxed)
}

fn urlencoding(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
            ' ' => "%20".to_string(),
            _ => format!("%{:02X}", c as u8),
        })
        .collect()
}

pub struct AppState {
    pub config: Mutex<Config>,
    pub translate_in_flight: Mutex<bool>,
    pub current_shortcut: Mutex<Option<Shortcut>>,
}

#[tauri::command]
fn get_config(state: tauri::State<'_, AppState>) -> Config {
    state.config.lock().unwrap().clone()
}

#[tauri::command]
async fn save_config(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
    new_config: Config,
) -> Result<(), String> {
    // Update hotkey if changed
    let old_hotkey = state.config.lock().unwrap().hotkey.clone();
    if old_hotkey != new_config.hotkey {
        update_hotkey(&app, &state, &new_config.hotkey)?;
    }

    // Save config
    *state.config.lock().unwrap() = new_config.clone();
    config::save(&new_config).map_err(|e| e.to_string())?;

    info!(
        model = %new_config.model,
        target_language = %new_config.target_language,
        reasoning = new_config.reasoning_enabled,
        hotkey = %new_config.hotkey,
        "Settings saved"
    );

    show_toast(&app, "success", "Saved");
    Ok(())
}

#[tauri::command]
fn pause_hotkey(app: AppHandle, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let shortcut = state.current_shortcut.lock().unwrap();
    if let Some(s) = shortcut.as_ref() {
        app.global_shortcut()
            .unregister(*s)
            .map_err(|e| e.to_string())?;
        debug!("Hotkey paused for recording");
    }
    Ok(())
}

#[tauri::command]
fn resume_hotkey(app: AppHandle, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let shortcut = state.current_shortcut.lock().unwrap();
    if let Some(s) = shortcut.as_ref() {
        app.global_shortcut()
            .register(*s)
            .map_err(|e| e.to_string())?;
        debug!("Hotkey resumed after recording");
    }
    Ok(())
}

#[tauri::command]
async fn translate(app: AppHandle, state: tauri::State<'_, AppState>) -> Result<(), String> {
    {
        let in_flight = state.translate_in_flight.lock().unwrap();
        if *in_flight {
            debug!("Translation requested while busy");
            show_toast(&app, "error", "Busy");
            return Err("Translation already in progress".to_string());
        }
    }

    // Read clipboard
    let input = app
        .clipboard()
        .read_text()
        .map_err(|e| {
            error!(error = %e, "Clipboard read failed");
            show_toast(&app, "error", "Clipboard failed");
            e.to_string()
        })?;

    if input.trim().is_empty() {
        debug!("Clipboard was empty");
        show_toast(&app, "error", "Clipboard empty");
        return Err("Clipboard is empty".to_string());
    }

    let config = state.config.lock().unwrap().clone();
    if config.target_language.trim().is_empty() {
        debug!("Missing target language");
        show_toast(&app, "error", "Missing language");
        return Err("Target language not set".to_string());
    }

    // Mark as in-flight
    *state.translate_in_flight.lock().unwrap() = true;
    show_toast(&app, "processing", "");
    let request_id = next_request_id();
    let span = tracing::info_span!(
        "translation",
        request_id,
        model = %config.model,
        target_language = %config.target_language,
        reasoning = config.reasoning_enabled,
        input_len = input.len()
    );
    span.in_scope(|| {
        info!("Translation triggered");
    });

    let result = openrouter::translate(&config, &input)
        .instrument(span.clone())
        .await;

    // Mark as complete
    *state.translate_in_flight.lock().unwrap() = false;

    span.in_scope(|| match result {
        Ok(translated) => {
            app.clipboard()
                .write_text(&translated)
                .map_err(|e| {
                    error!(error = %e, "Clipboard write failed");
                    show_toast(&app, "error", "Clipboard failed");
                    e.to_string()
                })?;
            info!(translated_len = translated.len(), "Translation applied");
            show_toast(&app, "success", "");
            Ok(())
        }
        Err(e) => {
            error!(error = %e, "Translation failed");
            show_toast(&app, "error", "");
            Err(e.to_string())
        }
    })
}

fn show_toast(app: &AppHandle, kind: &str, title: &str) {
    const TOAST_WIDTH: f64 = 200.0;
    const TOAST_HEIGHT: f64 = 56.0;
    const MARGIN: f64 = 16.0;
    const TASKBAR_HEIGHT: f64 = 48.0;

    // Create or get toast window
    let (toast, is_new) = match app.get_webview_window("toast") {
        Some(w) => (w, false),
        None => {
            // Pass initial state via URL query params
            let url = format!("toast.html?kind={}&title={}", kind, urlencoding(title));
            match WebviewWindowBuilder::new(app, "toast", WebviewUrl::App(url.into()))
                .title("Toast")
                .decorations(false)
                .transparent(true)
                .shadow(false)
                .always_on_top(true)
                .skip_taskbar(true)
                .resizable(false)
                .inner_size(TOAST_WIDTH, TOAST_HEIGHT)
                .visible(false)
                .build()
            {
                Ok(w) => (w, true),
                Err(e) => {
                    error!(error = %e, "Failed to create toast window");
                    return;
                }
            }
        }
    };

    // Position toast at bottom-right corner
    if let Some(monitor) = toast.primary_monitor().ok().flatten() {
        let screen_size = monitor.size();
        let scale = monitor.scale_factor();
        let screen_w = screen_size.width as f64 / scale;
        let screen_h = screen_size.height as f64 / scale;
        let x = screen_w - TOAST_WIDTH - MARGIN;
        let y = screen_h - TOAST_HEIGHT - MARGIN - TASKBAR_HEIGHT;
        let _ = toast.set_position(tauri::PhysicalPosition::new(
            (x * scale) as i32,
            (y * scale) as i32,
        ));
    }

    // Update toast content via event (only for existing windows)
    if !is_new {
        let _ = toast.emit("update-toast", serde_json::json!({
            "kind": kind,
            "title": title
        }));
    }

    // Show toast
    let _ = toast.show();

    // Schedule hide after duration
    let app_handle = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(TOAST_DURATION_MS));
        if let Some(toast) = app_handle.get_webview_window("toast") {
            let _ = toast.hide();
        }
    });
}

fn open_settings(app: &AppHandle) {
    if let Some(settings) = app.get_webview_window("settings") {
        let _ = settings.show();
        let _ = settings.set_focus();
        info!("Settings window reused");
        return;
    }

    match WebviewWindowBuilder::new(app, "settings", WebviewUrl::App("settings.html".into()))
        .title("ThirdSpace Settings")
        .inner_size(480.0, 520.0)
        .resizable(false)
        .maximizable(false)
        .center()
        .build()
    {
        Ok(_) => info!("Settings window opened"),
        Err(e) => {
            error!(error = %e, "Settings window failed");
            show_toast(app, "error", "Settings failed");
        }
    }
}

fn update_hotkey(
    app: &AppHandle,
    state: &tauri::State<'_, AppState>,
    hotkey_str: &str,
) -> Result<(), String> {
    let new_shortcut = parse_shortcut(hotkey_str)?;

    // Unregister old shortcut
    if let Some(old_shortcut) = state.current_shortcut.lock().unwrap().take() {
        let _ = app.global_shortcut().unregister(old_shortcut);
    }

    // Register new shortcut
    app.global_shortcut()
        .register(new_shortcut)
        .map_err(|e| format!("Failed to register hotkey: {}", e))?;

    *state.current_shortcut.lock().unwrap() = Some(new_shortcut);
    info!(hotkey = %hotkey_str, "Hotkey updated");
    Ok(())
}

fn parse_shortcut(input: &str) -> Result<Shortcut, String> {
    let tokens: Vec<&str> = input
        .split('+')
        .map(|t| t.trim())
        .filter(|t| !t.is_empty())
        .collect();

    let mut modifiers = Modifiers::empty();
    let mut key_code: Option<Code> = None;

    for token in tokens {
        let lower = token.to_ascii_lowercase();
        match lower.as_str() {
            "ctrl" | "control" => modifiers |= Modifiers::CONTROL,
            "alt" | "option" => modifiers |= Modifiers::ALT,
            "shift" => modifiers |= Modifiers::SHIFT,
            "win" | "super" | "meta" | "cmd" | "command" => modifiers |= Modifiers::SUPER,
            _ => {
                if key_code.is_some() {
                    return Err("Multiple keys specified".to_string());
                }
                key_code = Some(parse_key_code(&lower)?);
            }
        }
    }

    let code = key_code.ok_or_else(|| "No key specified".to_string())?;
    Ok(Shortcut::new(Some(modifiers), code))
}

fn parse_key_code(key: &str) -> Result<Code, String> {
    // Single letter
    if key.len() == 1 {
        let ch = key.chars().next().unwrap();
        if ch.is_ascii_alphabetic() {
            return match ch {
                'a' => Ok(Code::KeyA),
                'b' => Ok(Code::KeyB),
                'c' => Ok(Code::KeyC),
                'd' => Ok(Code::KeyD),
                'e' => Ok(Code::KeyE),
                'f' => Ok(Code::KeyF),
                'g' => Ok(Code::KeyG),
                'h' => Ok(Code::KeyH),
                'i' => Ok(Code::KeyI),
                'j' => Ok(Code::KeyJ),
                'k' => Ok(Code::KeyK),
                'l' => Ok(Code::KeyL),
                'm' => Ok(Code::KeyM),
                'n' => Ok(Code::KeyN),
                'o' => Ok(Code::KeyO),
                'p' => Ok(Code::KeyP),
                'q' => Ok(Code::KeyQ),
                'r' => Ok(Code::KeyR),
                's' => Ok(Code::KeyS),
                't' => Ok(Code::KeyT),
                'u' => Ok(Code::KeyU),
                'v' => Ok(Code::KeyV),
                'w' => Ok(Code::KeyW),
                'x' => Ok(Code::KeyX),
                'y' => Ok(Code::KeyY),
                'z' => Ok(Code::KeyZ),
                _ => Err(format!("Unknown key: {}", key)),
            };
        }
        if ch.is_ascii_digit() {
            return match ch {
                '0' => Ok(Code::Digit0),
                '1' => Ok(Code::Digit1),
                '2' => Ok(Code::Digit2),
                '3' => Ok(Code::Digit3),
                '4' => Ok(Code::Digit4),
                '5' => Ok(Code::Digit5),
                '6' => Ok(Code::Digit6),
                '7' => Ok(Code::Digit7),
                '8' => Ok(Code::Digit8),
                '9' => Ok(Code::Digit9),
                _ => Err(format!("Unknown key: {}", key)),
            };
        }
    }

    // Function keys
    if key.starts_with('f') && key.len() >= 2 {
        if let Ok(num) = key[1..].parse::<u8>() {
            return match num {
                1 => Ok(Code::F1),
                2 => Ok(Code::F2),
                3 => Ok(Code::F3),
                4 => Ok(Code::F4),
                5 => Ok(Code::F5),
                6 => Ok(Code::F6),
                7 => Ok(Code::F7),
                8 => Ok(Code::F8),
                9 => Ok(Code::F9),
                10 => Ok(Code::F10),
                11 => Ok(Code::F11),
                12 => Ok(Code::F12),
                _ => Err(format!("Unknown function key: {}", key)),
            };
        }
    }

    // Special keys
    match key {
        "space" | "spacebar" => Ok(Code::Space),
        "enter" | "return" => Ok(Code::Enter),
        "tab" => Ok(Code::Tab),
        "esc" | "escape" => Ok(Code::Escape),
        "backspace" => Ok(Code::Backspace),
        "delete" | "del" => Ok(Code::Delete),
        "insert" | "ins" => Ok(Code::Insert),
        "home" => Ok(Code::Home),
        "end" => Ok(Code::End),
        "pageup" | "pgup" => Ok(Code::PageUp),
        "pagedown" | "pgdn" => Ok(Code::PageDown),
        "up" | "arrowup" => Ok(Code::ArrowUp),
        "down" | "arrowdown" => Ok(Code::ArrowDown),
        "left" | "arrowleft" => Ok(Code::ArrowLeft),
        "right" | "arrowright" => Ok(Code::ArrowRight),
        _ => Err(format!("Unknown key: {}", key)),
    }
}

fn build_log_filter() -> tracing_subscriber::EnvFilter {
    tracing_subscriber::EnvFilter::try_from_env("THIRDSPACE_LOG")
        .or_else(|_| tracing_subscriber::EnvFilter::try_from_default_env())
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(DEFAULT_LOG_FILTER))
}

fn cleanup_old_logs(log_dir: &Path) {
    let cutoff = match SystemTime::now()
        .checked_sub(Duration::from_secs(LOG_RETENTION_DAYS * 24 * 60 * 60))
    {
        Some(time) => time,
        None => return,
    };

    let entries = match std::fs::read_dir(log_dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let file_name = match path.file_name().and_then(|name| name.to_str()) {
            Some(name) => name,
            None => continue,
        };
        if !file_name.starts_with(LOG_FILE_PREFIX) {
            continue;
        }
        let modified = match entry.metadata().and_then(|meta| meta.modified()) {
            Ok(time) => time,
            Err(_) => continue,
        };
        if modified < cutoff {
            let _ = std::fs::remove_file(path);
        }
    }
}

fn setup_logging() -> Option<tracing_appender::non_blocking::WorkerGuard> {
    let log_dir = config::logs_dir().ok()?;
    let filter = build_log_filter();
    if std::fs::create_dir_all(&log_dir).is_err() {
        let _ = tracing_subscriber::fmt()
            .with_ansi(false)
            .with_env_filter(build_log_filter())
            .try_init();
        return None;
    }

    cleanup_old_logs(&log_dir);

    let file_appender = tracing_appender::rolling::daily(&log_dir, LOG_FILE_PREFIX);
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::fmt()
        .with_writer(non_blocking)
        .with_ansi(false)
        .with_target(true)
        .with_level(true)
        .with_thread_ids(true)
        .with_thread_names(true)
        .with_file(true)
        .with_line_number(true)
        .with_env_filter(filter)
        .init();

    info!(
        log_dir = %log_dir.display(),
        retention_days = LOG_RETENTION_DAYS,
        "Logging initialized"
    );
    Some(guard)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let migrate_result = config::migrate_legacy_data();
    let _log_guard = setup_logging();
    if let Err(err) = migrate_result {
        error!(error = %err, "Legacy data migration failed");
    }

    let config = config::load().unwrap_or_default();
    let initial_hotkey = config.hotkey.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    if event.state == ShortcutState::Pressed {
                        let state = app.state::<AppState>();
                        let is_our_shortcut = {
                            let guard = state.current_shortcut.lock().unwrap();
                            guard.as_ref().map_or(false, |current| shortcut == current)
                        };
                        if is_our_shortcut {
                            let app = app.clone();
                            tauri::async_runtime::spawn(async move {
                                let state = app.state::<AppState>();
                                let _ = translate(app.clone(), state).await;
                            });
                        }
                    }
                })
                .build(),
        )
        .manage(AppState {
            config: Mutex::new(config),
            translate_in_flight: Mutex::new(false),
            current_shortcut: Mutex::new(None),
        })
        .setup(move |app| {
            // Setup system tray
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let settings = MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?;
            let translate_item =
                MenuItem::with_id(app, "translate", "Translate", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&translate_item, &settings, &quit])?;

            TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "translate" => {
                        let app = app.clone();
                        tauri::async_runtime::spawn(async move {
                            let state = app.state::<AppState>();
                            let _ = translate(app.clone(), state).await;
                        });
                    }
                    "settings" => {
                        open_settings(app);
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
                .build(app)?;

            // Register initial hotkey
            let state = app.state::<AppState>();
            if let Ok(shortcut) = parse_shortcut(&initial_hotkey) {
                if app.global_shortcut().register(shortcut).is_ok() {
                    *state.current_shortcut.lock().unwrap() = Some(shortcut);
                    info!(hotkey = %initial_hotkey, "Hotkey registered");
                }
            }

            info!("ThirdSpace started");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![get_config, save_config, translate, pause_hotkey, resume_hotkey])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app, event| {
            // Only prevent exit when closing windows (code is None)
            // Allow exit when explicitly called via app.exit() (code is Some)
            if let tauri::RunEvent::ExitRequested { api, code, .. } = event {
                if code.is_none() {
                    api.prevent_exit();
                }
            }
        });
}
