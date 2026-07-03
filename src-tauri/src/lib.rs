pub mod action;
mod audio;
pub mod engine;
pub mod layout;
mod mic;
mod pipeline;
pub mod routine;

use std::sync::{Arc, Mutex};

use pipeline::{Engine, EngineEvent};
use routine::{Routine, RoutineConfig, RoutineStore};
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
fn check_accessibility_permission(prompt: bool) -> bool {
    layout::is_trusted(prompt)
}

/// Build the tray menu from the current app state. The menu is rebuilt
/// wholesale on every state change instead of mutating item handles, so the
/// dynamic routine section can never go stale.
fn build_tray_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let status = *app.state::<StatusState>().0.lock().unwrap();
    let detection_enabled = app.state::<EngineState>().0.is_detection_enabled();
    let autostart_enabled = app.autolaunch().is_enabled().unwrap_or(false);
    let config = app.state::<StoreState>().0.snapshot();

    let menu = Menu::new(app)?;
    menu.append(&MenuItem::with_id(
        app,
        "status",
        status.label(),
        false,
        None::<&str>,
    )?)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;

    menu.append(&MenuItem::with_id(
        app,
        "routines_header",
        "Active routine",
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
            "No routines yet",
            false,
            None::<&str>,
        )?)?;
    }
    menu.append(&PredefinedMenuItem::separator(app)?)?;

    menu.append(&MenuItem::with_id(
        app,
        "show",
        "Show settings",
        true,
        None::<&str>,
    )?)?;
    menu.append(&CheckMenuItem::with_id(
        app,
        "detection",
        "Detection enabled",
        true,
        detection_enabled,
        None::<&str>,
    )?)?;
    menu.append(&CheckMenuItem::with_id(
        app,
        "autostart",
        "Auto-start on login",
        true,
        autostart_enabled,
        None::<&str>,
    )?)?;
    menu.append(&MenuItem::with_id(
        app,
        "test_mic",
        "Test microphone",
        true,
        None::<&str>,
    )?)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?)?;

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
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            None,
        ))
        .invoke_handler(tauri::generate_handler![
            list_routines,
            save_routine,
            delete_routine,
            set_active_routine,
            check_accessibility_permission
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
                        action::run_routine(&actions);
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
                        "test_mic" => match mic::request_microphone() {
                            Ok(device_name) => {
                                println!("[mic] access granted, device={device_name}");
                            }
                            Err(err) => {
                                eprintln!("[mic] access failed: {err}");
                                set_status(app, AppStatus::MicPermissionMissing);
                            }
                        },
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
