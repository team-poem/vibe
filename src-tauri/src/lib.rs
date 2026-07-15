pub mod action;
mod audio;
pub mod engine;
pub mod layout;
mod mic;
mod pipeline;
pub mod routine;

use std::sync::{Arc, Mutex};

use engine::Sensitivity;
use pipeline::{Engine, EngineEvent};
use routine::{
    ExecutionLog, ExecutionRecord, Language, Routine, RoutineConfig, RoutineStore, Theme,
};
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager, Runtime, WindowEvent};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};

const TRAY_ID: &str = "main";
const ROUTINE_MENU_PREFIX: &str = "routine:";

#[derive(Debug, Clone, Copy)]
enum AppStatus {
    Waiting,
    DetectionPaused,
    MicPermissionMissing,
}

impl AppStatus {
    fn label(self, language: Language) -> &'static str {
        match (self, language) {
            (AppStatus::Waiting, Language::En) => "Status: Waiting",
            (AppStatus::Waiting, Language::Ko) => "상태: 대기 중",
            (AppStatus::DetectionPaused, Language::En) => "Status: Detection paused",
            (AppStatus::DetectionPaused, Language::Ko) => "상태: 감지 일시정지",
            (AppStatus::MicPermissionMissing, Language::En) => "Status: Mic permission missing",
            (AppStatus::MicPermissionMissing, Language::Ko) => "상태: 마이크 권한 없음",
        }
    }
}

/// Fixed tray menu strings in both supported languages.
struct TrayText {
    active_routine: &'static str,
    no_routines: &'static str,
    show_settings: &'static str,
    detection: &'static str,
    autostart: &'static str,
    quit: &'static str,
}

fn tray_text(language: Language) -> TrayText {
    match language {
        Language::En => TrayText {
            active_routine: "Active routine",
            no_routines: "No routines yet",
            show_settings: "Show settings",
            detection: "Detection enabled",
            autostart: "Auto-start on login",
            quit: "Quit",
        },
        Language::Ko => TrayText {
            active_routine: "활성 루틴",
            no_routines: "루틴 없음",
            show_settings: "설정 열기",
            detection: "감지 활성화",
            autostart: "로그인 시 자동 실행",
            quit: "종료",
        },
    }
}

struct EngineState(Engine);
struct StatusState(Mutex<AppStatus>);
struct StoreState(Arc<RoutineStore>);
struct LogState(Arc<ExecutionLog>);

/// Repeated double-claps while a routine is still assembling would queue
/// full re-runs (relaunch, re-place, restack) and make every window flash
/// again — swallow triggers inside the cooldown window.
static LAST_RUN_STARTED_MS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
const TRIGGER_COOLDOWN_MS: u64 = 5000;

fn epoch_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[tauri::command]
fn list_routines(store: tauri::State<'_, StoreState>) -> RoutineConfig {
    store.0.snapshot()
}

#[tauri::command]
fn save_routine(
    app: AppHandle,
    store: tauri::State<'_, StoreState>,
    routine: Routine,
) -> Result<RoutineConfig, String> {
    let config = store.0.upsert_routine(routine).map_err(|e| e.to_string())?;
    notify_routines_changed(&app);
    Ok(config)
}

#[tauri::command]
fn delete_routine(
    app: AppHandle,
    store: tauri::State<'_, StoreState>,
    id: String,
) -> Result<RoutineConfig, String> {
    let config = store.0.delete_routine(&id).map_err(|e| e.to_string())?;
    notify_routines_changed(&app);
    Ok(config)
}

#[tauri::command]
fn set_active_routine(
    app: AppHandle,
    store: tauri::State<'_, StoreState>,
    id: Option<String>,
) -> Result<RoutineConfig, String> {
    let config = store.0.set_active_routine(id).map_err(|e| e.to_string())?;
    notify_routines_changed(&app);
    Ok(config)
}

#[tauri::command]
fn set_language(
    app: AppHandle,
    store: tauri::State<'_, StoreState>,
    language: Language,
) -> Result<RoutineConfig, String> {
    let config = store.0.set_language(language).map_err(|e| e.to_string())?;
    notify_routines_changed(&app);
    Ok(config)
}

#[tauri::command]
fn set_theme(store: tauri::State<'_, StoreState>, theme: Theme) -> Result<RoutineConfig, String> {
    store.0.set_theme(theme).map_err(|e| e.to_string())
}

#[tauri::command]
fn set_sensitivity(
    store: tauri::State<'_, StoreState>,
    engine: tauri::State<'_, EngineState>,
    sensitivity: Sensitivity,
) -> Result<RoutineConfig, String> {
    let config = store
        .0
        .set_sensitivity(sensitivity)
        .map_err(|e| e.to_string())?;
    engine.0.set_sensitivity(sensitivity);
    Ok(config)
}

#[tauri::command]
fn check_accessibility_permission(prompt: bool) -> bool {
    layout::is_trusted(prompt)
}

/// macOS may not surface a newly granted Accessibility permission to an
/// already-running process; restarting the app is the reliable way out.
#[tauri::command]
fn restart_app(app: AppHandle) {
    app.restart();
}

/// Unsigned builds get a new code identity every rebuild, leaving a stale
/// TCC row that reports "granted" while every call is blocked. Clear our
/// own entry, then re-prompt so a fresh grant matches this binary.
#[tauri::command]
fn repair_accessibility_permission() -> bool {
    let _ = std::process::Command::new("tccutil")
        .args(["reset", "Accessibility", "com.vibe.app"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    layout::is_trusted(true)
}

#[tauri::command]
fn list_execution_log(log: tauri::State<'_, LogState>) -> Vec<ExecutionRecord> {
    log.0.snapshot()
}

#[tauri::command]
fn get_autostart(app: AppHandle) -> bool {
    app.autolaunch().is_enabled().unwrap_or(false)
}

#[tauri::command]
fn set_autostart(app: AppHandle, enabled: bool) -> Result<bool, String> {
    let autostart = app.autolaunch();
    let outcome = if enabled {
        autostart.enable()
    } else {
        autostart.disable()
    };
    outcome.map_err(|e| e.to_string())?;
    refresh_tray_menu(&app);
    Ok(app.autolaunch().is_enabled().unwrap_or(false))
}

/// Opens the input device, which triggers the macOS microphone permission
/// dialog on first use. Returns the device name on success.
#[tauri::command]
fn test_microphone() -> Result<String, String> {
    mic::request_microphone()
}

#[tauri::command]
fn list_displays() -> Vec<layout::DisplayInfo> {
    layout::list_displays()
}

/// Names of installed applications, for the app-name autocomplete in the
/// routine editor.
#[tauri::command]
fn list_installed_apps() -> Vec<String> {
    let home = std::env::var("HOME").unwrap_or_default();
    let dirs = [
        "/Applications".to_owned(),
        "/System/Applications".to_owned(),
        format!("{home}/Applications"),
    ];

    let mut apps = std::collections::BTreeSet::new();
    for dir in &dirs {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if let Some(app) = name.strip_suffix(".app") {
                apps.insert(app.to_owned());
            }
        }
    }
    apps.into_iter().collect()
}

/// Build the tray menu from the current app state. The menu is rebuilt
/// wholesale on every state change instead of mutating item handles, so the
/// dynamic routine section can never go stale.
fn build_tray_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let status = *app.state::<StatusState>().0.lock().unwrap();
    let detection_enabled = app.state::<EngineState>().0.is_detection_enabled();
    let autostart_enabled = app.autolaunch().is_enabled().unwrap_or(false);
    let config = app.state::<StoreState>().0.snapshot();
    let language = config.language.unwrap_or_default();
    let text = tray_text(language);

    let menu = Menu::new(app)?;
    menu.append(&MenuItem::with_id(
        app,
        "status",
        status.label(language),
        false,
        None::<&str>,
    )?)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;

    menu.append(&MenuItem::with_id(
        app,
        "routines_header",
        text.active_routine,
        false,
        None::<&str>,
    )?)?;
    for routine in &config.routines {
        let is_active = config.active_routine_id.as_deref() == Some(routine.id.as_str());
        menu.append(&CheckMenuItem::with_id(
            app,
            format!("{ROUTINE_MENU_PREFIX}{}", routine.id),
            &routine.name,
            true,
            is_active,
            None::<&str>,
        )?)?;
    }
    if config.routines.is_empty() {
        menu.append(&MenuItem::with_id(
            app,
            "routines_empty",
            text.no_routines,
            false,
            None::<&str>,
        )?)?;
    }
    menu.append(&PredefinedMenuItem::separator(app)?)?;

    menu.append(&MenuItem::with_id(
        app,
        "show",
        text.show_settings,
        true,
        None::<&str>,
    )?)?;
    menu.append(&CheckMenuItem::with_id(
        app,
        "detection",
        text.detection,
        true,
        detection_enabled,
        None::<&str>,
    )?)?;
    menu.append(&CheckMenuItem::with_id(
        app,
        "autostart",
        text.autostart,
        true,
        autostart_enabled,
        None::<&str>,
    )?)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&MenuItem::with_id(
        app,
        "quit",
        text.quit,
        true,
        None::<&str>,
    )?)?;

    Ok(menu)
}

/// Fan out a routine document change: the tray menu is rebuilt and the
/// webview (if open) is told to refetch.
fn notify_routines_changed<R: Runtime>(app: &AppHandle<R>) {
    let _ = app.emit("routines://changed", ());
    refresh_tray_menu(app);
}

fn refresh_tray_menu<R: Runtime>(app: &AppHandle<R>) {
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return;
    };
    match build_tray_menu(app) {
        Ok(menu) => {
            let _ = tray.set_menu(Some(menu));
        }
        Err(err) => eprintln!("[tray] failed to rebuild menu: {err}"),
    }
}

fn set_status<R: Runtime>(app: &AppHandle<R>, status: AppStatus) {
    *app.state::<StatusState>().0.lock().unwrap() = status;
    refresh_tray_menu(app);
}

fn handle_routine_menu_click<R: Runtime>(app: &AppHandle<R>, routine_id: &str) {
    let store = app.state::<StoreState>();
    let currently_active = store.0.snapshot().active_routine_id;
    // Clicking the active routine deactivates it; anything else activates.
    let next = if currently_active.as_deref() == Some(routine_id) {
        None
    } else {
        Some(routine_id.to_owned())
    };
    if let Err(err) = store.0.set_active_routine(next) {
        eprintln!("[tray] failed to switch active routine: {err}");
    }
    notify_routines_changed(app);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Two live instances would each hear the same clap and run the
        // routine twice, racing each other's window snapshots (a stale
        // instance once dragged another display's tab group fullscreen).
        // Registered first so the guard runs before any other setup.
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            layout::log_place("[app] second launch blocked — showing existing instance");
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            None,
        ))
        .invoke_handler(tauri::generate_handler![
            list_routines,
            save_routine,
            delete_routine,
            set_active_routine,
            set_language,
            set_theme,
            set_sensitivity,
            check_accessibility_permission,
            repair_accessibility_permission,
            restart_app,
            list_execution_log,
            get_autostart,
            set_autostart,
            test_microphone,
            list_installed_apps,
            list_displays
        ])
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let (store, load_report) =
                RoutineStore::load_or_recover(data_dir.join("routines.json"));
            println!("[routine] store ready: {load_report:?}");
            let store = Arc::new(store);

            let execution_log = Arc::new(ExecutionLog::new());

            let trigger_store = store.clone();
            let trigger_log = execution_log.clone();
            let trigger_app = app.handle().clone();
            let initial_sensitivity = store.snapshot().sensitivity;
            let engine = pipeline::start(initial_sensitivity, move |event| match event {
                EngineEvent::Trigger(trigger) => {
                    layout::log_place(&format!(
                        "[trigger] double clap interval={}ms confidence={:.2}",
                        trigger.interval_ms, trigger.confidence
                    ));
                    let now = epoch_ms();
                    let last = LAST_RUN_STARTED_MS.load(std::sync::atomic::Ordering::Relaxed);
                    if now.saturating_sub(last) < TRIGGER_COOLDOWN_MS {
                        layout::log_place("[trigger] ignored — cooldown");
                        return;
                    }
                    let Some(routine) = trigger_store.snapshot().active_routine().cloned() else {
                        println!("[routine] no active routine, trigger ignored");
                        return;
                    };
                    if action::routine_already_assembled(&routine.actions) {
                        layout::log_place("[trigger] ignored — routine already assembled");
                        return;
                    }
                    LAST_RUN_STARTED_MS.store(now, std::sync::atomic::Ordering::Relaxed);
                    let outcomes = action::run_routine(&routine.actions);
                    trigger_log.push(ExecutionRecord {
                        at_epoch_ms: epoch_ms(),
                        routine_name: routine.name,
                        success: outcomes.iter().all(|o| o.success),
                        outcomes,
                    });
                    let _ = trigger_app.emit("exec-log://updated", ());
                }
                EngineEvent::CaptureFailed(message) => {
                    eprintln!("[audio] capture failed: {message}");
                    let app = trigger_app.clone();
                    let _ = app.clone().run_on_main_thread(move || {
                        set_status(&app, AppStatus::MicPermissionMissing);
                    });
                }
            });
            app.manage(EngineState(engine));
            app.manage(StoreState(store));
            app.manage(LogState(execution_log));
            app.manage(StatusState(Mutex::new(AppStatus::Waiting)));

            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let menu = build_tray_menu(app.handle())?;
            let _tray = TrayIconBuilder::with_id(TRAY_ID)
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .show_menu_on_left_click(true)
                .on_menu_event(move |app, event| {
                    let id = event.id.as_ref();
                    if let Some(routine_id) = id.strip_prefix(ROUTINE_MENU_PREFIX) {
                        handle_routine_menu_click(app, routine_id);
                        return;
                    }
                    match id {
                        "show" => {
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                        "detection" => {
                            let enabled = {
                                let state = app.state::<EngineState>();
                                let next = !state.0.is_detection_enabled();
                                state.0.set_detection_enabled(next);
                                next
                            };
                            let next_status = if enabled {
                                AppStatus::Waiting
                            } else {
                                AppStatus::DetectionPaused
                            };
                            set_status(app, next_status);
                            println!(
                                "[detection] {}",
                                if enabled { "enabled" } else { "disabled" }
                            );
                        }
                        "autostart" => {
                            let autostart = app.autolaunch();
                            let target = !autostart.is_enabled().unwrap_or(false);
                            let outcome = if target {
                                autostart.enable()
                            } else {
                                autostart.disable()
                            };
                            if let Err(err) = outcome {
                                eprintln!("[autostart] failed to toggle: {err}");
                            }
                            refresh_tray_menu(app);
                        }
                        "quit" => {
                            app.state::<EngineState>().0.shutdown();
                            app.exit(0);
                        }
                        _ => {}
                    }
                })
                .build(app)?;

            if let Some(window) = app.get_webview_window("main") {
                let window_handle = window.clone();
                window.on_window_event(move |event| {
                    if let WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = window_handle.hide();
                    }
                });
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
