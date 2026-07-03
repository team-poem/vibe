pub mod action;
mod audio;
pub mod engine;
mod mic;
mod pipeline;
pub mod routine;

use std::sync::{Arc, Mutex};

use action::Action;
use pipeline::{Engine, EngineEvent};
use routine::{Routine, RoutineConfig, RoutineStore};
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{Manager, WindowEvent};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};

#[derive(Debug, Clone, Copy)]
enum AppStatus {
    Waiting,
    DetectionPaused,
    MicPermissionMissing,
}

impl AppStatus {
    fn label(self) -> &'static str {
        match self {
            AppStatus::Waiting => "Status: Waiting",
            AppStatus::DetectionPaused => "Status: Detection paused",
            AppStatus::MicPermissionMissing => "Status: Mic permission missing",
        }
    }
}

struct EngineState(Engine);
struct StatusState(Mutex<AppStatus>);
struct StoreState(Arc<RoutineStore>);

/// Run a routine's actions sequentially (MVP execution policy) and log
/// each outcome. Runs on the engine's event worker thread.
fn run_routine(actions: &[Action]) {
    for action in actions {
        match action::run(action) {
            Ok(result) if result.exit_status.success() => println!(
                "[routine] {} done in {:.0} ms",
                result.action, result.dispatch_ms
            ),
            Ok(result) => eprintln!(
                "[routine] {} exited with {}",
                result.action, result.exit_status
            ),
            Err(err) => eprintln!("[routine] {err}"),
        }
    }
}

#[tauri::command]
fn list_routines(store: tauri::State<'_, StoreState>) -> RoutineConfig {
    store.0.snapshot()
}

#[tauri::command]
fn save_routine(
    store: tauri::State<'_, StoreState>,
    routine: Routine,
) -> Result<RoutineConfig, String> {
    store.0.upsert_routine(routine).map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_routine(
    store: tauri::State<'_, StoreState>,
    id: String,
) -> Result<RoutineConfig, String> {
    store.0.delete_routine(&id).map_err(|e| e.to_string())
}

#[tauri::command]
fn set_active_routine(
    store: tauri::State<'_, StoreState>,
    id: Option<String>,
) -> Result<RoutineConfig, String> {
    store.0.set_active_routine(id).map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            None,
        ))
        .invoke_handler(tauri::generate_handler![
            list_routines,
            save_routine,
            delete_routine,
            set_active_routine
        ])
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let (store, load_report) =
                RoutineStore::load_or_recover(data_dir.join("routines.json"));
            println!("[routine] store ready: {load_report:?}");
            let store = Arc::new(store);

            let trigger_store = store.clone();
            let engine = pipeline::start(move |event| match event {
                EngineEvent::Trigger(trigger) => {
                    println!(
                        "[trigger] double clap interval={}ms confidence={:.2}",
                        trigger.interval_ms, trigger.confidence
                    );
                    let actions = trigger_store.active_actions();
                    if actions.is_empty() {
                        println!("[routine] no active routine, trigger ignored");
                    } else {
                        run_routine(&actions);
                    }
                }
                EngineEvent::CaptureFailed(message) => {
                    eprintln!("[audio] capture failed: {message}");
                }
            });
            app.manage(EngineState(engine));
            app.manage(StoreState(store));
            app.manage(StatusState(Mutex::new(AppStatus::Waiting)));

            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let autostart = app.autolaunch();
            let autostart_initial = autostart.is_enabled().unwrap_or(false);

            let status_item = MenuItem::with_id(
                app,
                "status",
                AppStatus::Waiting.label(),
                false,
                None::<&str>,
            )?;
            let status_separator = PredefinedMenuItem::separator(app)?;
            let show_item = MenuItem::with_id(app, "show", "Show settings", true, None::<&str>)?;
            let detection_item = CheckMenuItem::with_id(
                app,
                "detection",
                "Detection enabled",
                true,
                true,
                None::<&str>,
            )?;
            let autostart_item = CheckMenuItem::with_id(
                app,
                "autostart",
                "Auto-start on login",
                true,
                autostart_initial,
                None::<&str>,
            )?;
            let test_mic_item =
                MenuItem::with_id(app, "test_mic", "Test microphone", true, None::<&str>)?;
            let bottom_separator = PredefinedMenuItem::separator(app)?;
            let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(
                app,
                &[
                    &status_item,
                    &status_separator,
                    &show_item,
                    &detection_item,
                    &autostart_item,
                    &test_mic_item,
                    &bottom_separator,
                    &quit_item,
                ],
            )?;

            let status_handle = status_item.clone();
            let detection_handle = detection_item.clone();
            let autostart_handle = autostart_item.clone();

            let _tray = TrayIconBuilder::with_id("main")
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .show_menu_on_left_click(true)
                .on_menu_event(move |app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "detection" => {
                        let state = app.state::<EngineState>();
                        let enabled = !state.0.is_detection_enabled();
                        state.0.set_detection_enabled(enabled);
                        let _ = detection_handle.set_checked(enabled);
                        let next = if enabled {
                            AppStatus::Waiting
                        } else {
                            AppStatus::DetectionPaused
                        };
                        apply_status(app, &status_handle, next);
                        println!(
                            "[detection] {}",
                            if enabled { "enabled" } else { "disabled" }
                        );
                    }
                    "autostart" => {
                        let autostart = app.autolaunch();
                        let currently_enabled = autostart.is_enabled().unwrap_or(false);
                        let target = !currently_enabled;
                        let outcome = if target {
                            autostart.enable()
                        } else {
                            autostart.disable()
                        };
                        match outcome {
                            Ok(_) => {
                                let _ = autostart_handle.set_checked(target);
                                println!(
                                    "[autostart] {}",
                                    if target { "enabled" } else { "disabled" }
                                );
                            }
                            Err(err) => {
                                eprintln!("[autostart] failed to toggle: {err}");
                                let _ = autostart_handle.set_checked(currently_enabled);
                            }
                        }
                    }
                    "test_mic" => match mic::request_microphone() {
                        Ok(device_name) => {
                            println!("[mic] access granted, device={device_name}");
                        }
                        Err(err) => {
                            eprintln!("[mic] access failed: {err}");
                            apply_status(app, &status_handle, AppStatus::MicPermissionMissing);
                        }
                    },
                    "quit" => {
                        app.state::<EngineState>().0.shutdown();
                        app.exit(0);
                    }
                    _ => {}
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

fn apply_status<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    handle: &MenuItem<R>,
    status: AppStatus,
) {
    let state = app.state::<StatusState>();
    *state.0.lock().unwrap() = status;
    let _ = handle.set_text(status.label());
}
