import { invoke } from "@tauri-apps/api/core";

import type { Language } from "../../shared/i18n/messages";
import type { ThemeSetting } from "../../shared/theme";
import type { ExecutionRecord, Routine, RoutineConfig } from "./types";

export async function fetchRoutineConfig(): Promise<RoutineConfig> {
  return invoke<RoutineConfig>("list_routines");
}

export async function saveRoutineToStore(
  routine: Routine,
): Promise<RoutineConfig> {
  return invoke<RoutineConfig>("save_routine", { routine });
}

export async function deleteRoutineFromStore(
  id: string,
): Promise<RoutineConfig> {
  return invoke<RoutineConfig>("delete_routine", { id });
}

export async function setActiveRoutineInStore(
  id: string | null,
): Promise<RoutineConfig> {
  return invoke<RoutineConfig>("set_active_routine", { id });
}

/// With `prompt`, macOS shows the Accessibility permission dialog and adds
/// the app to the System Settings list.
export async function checkAccessibilityPermission(
  prompt: boolean,
): Promise<boolean> {
  return invoke<boolean>("check_accessibility_permission", { prompt });
}

/// Recent routine runs, newest first.
export async function fetchExecutionLog(): Promise<ExecutionRecord[]> {
  return invoke<ExecutionRecord[]>("list_execution_log");
}

export async function setLanguageInStore(
  language: Language,
): Promise<RoutineConfig> {
  return invoke<RoutineConfig>("set_language", { language });
}

export async function setThemeInStore(
  theme: ThemeSetting,
): Promise<RoutineConfig> {
  return invoke<RoutineConfig>("set_theme", { theme });
}

export async function fetchAutostart(): Promise<boolean> {
  return invoke<boolean>("get_autostart");
}

export async function setAutostartInSystem(enabled: boolean): Promise<boolean> {
  return invoke<boolean>("set_autostart", { enabled });
}

/// Opens the input device; triggers the macOS mic permission dialog on
/// first use. Resolves to the device name.
export async function testMicrophone(): Promise<string> {
  return invoke<string>("test_microphone");
}

/// Installed application names, for the app-name autocomplete.
export async function fetchInstalledApps(): Promise<string[]> {
  return invoke<string[]>("list_installed_apps");
}
