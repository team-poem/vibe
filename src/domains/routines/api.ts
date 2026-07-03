import { invoke } from "@tauri-apps/api/core";

import type { Routine, RoutineConfig } from "./types";

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
