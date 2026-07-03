import { useEffect, useState } from "react";

import {
  deleteRoutineFromStore,
  fetchRoutineConfig,
  saveRoutineToStore,
  setActiveRoutineInStore,
} from "./api";
import type { Routine, RoutineConfig } from "./types";

interface UseRoutinesResult {
  config: RoutineConfig | null;
  error: string | null;
  saveRoutine: (routine: Routine) => Promise<Routine | null>;
  deleteRoutine: (id: string) => Promise<void>;
  setActiveRoutine: (id: string | null) => Promise<void>;
}

/// Owns the routine document mirrored from the Rust store. Every mutation
/// replaces the whole config with the store's response, so the UI can never
/// drift from the persisted file.
export const useRoutines = (): UseRoutinesResult => {
  const [config, setConfig] = useState<RoutineConfig | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    async function loadInitialConfig() {
      try {
        setConfig(await fetchRoutineConfig());
      } catch (cause) {
        setError(String(cause));
      }
    }
    void loadInitialConfig();
  }, []);

  async function saveRoutine(routine: Routine): Promise<Routine | null> {
    try {
      const next = await saveRoutineToStore(routine);
      setConfig(next);
      setError(null);
      // The store assigns ids to new routines; report the saved entity back
      // so callers can keep their selection on it.
      const saved =
        routine.id === ""
          ? findNewRoutine(config, next)
          : (next.routines.find((r) => r.id === routine.id) ?? null);
      return saved;
    } catch (cause) {
      setError(String(cause));
      return null;
    }
  }

  async function deleteRoutine(id: string) {
    try {
      setConfig(await deleteRoutineFromStore(id));
      setError(null);
    } catch (cause) {
      setError(String(cause));
    }
  }

  async function setActiveRoutine(id: string | null) {
    try {
      setConfig(await setActiveRoutineInStore(id));
      setError(null);
    } catch (cause) {
      setError(String(cause));
    }
  }

  return { config, error, saveRoutine, deleteRoutine, setActiveRoutine };
};

const findNewRoutine = (
  before: RoutineConfig | null,
  after: RoutineConfig,
): Routine | null => {
  const knownIds = new Set(before?.routines.map((r) => r.id) ?? []);
  return after.routines.find((r) => !knownIds.has(r.id)) ?? null;
};
