import { useEffect, useState } from "react";

import { listen } from "@tauri-apps/api/event";

import type { Language } from "../../shared/i18n/messages";
import {
  deleteRoutineFromStore,
  fetchRoutineConfig,
  saveRoutineToStore,
  setActiveRoutineInStore,
  setLanguageInStore,
} from "./api";
import type { Routine, RoutineConfig } from "./types";

interface UseRoutinesResult {
  config: RoutineConfig | null;
  error: string | null;
  saveRoutine: (routine: Routine) => Promise<Routine | null>;
  deleteRoutine: (id: string) => Promise<void>;
  setActiveRoutine: (id: string | null) => Promise<void>;
  setLanguage: (language: Language) => Promise<void>;
}

/// Owns the routine document mirrored from the Rust store. Every mutation
/// replaces the whole config with the store's response, so the UI can never
/// drift from the persisted file.
export const useRoutines = (): UseRoutinesResult => {
  const [config, setConfig] = useState<RoutineConfig | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    async function loadConfig() {
      try {
        setConfig(await fetchRoutineConfig());
      } catch (cause) {
        setError(String(cause));
      }
    }
    void loadConfig();

    // The tray menu can switch the active routine while this window is
    // open; the backend broadcasts document changes so both stay in sync.
    const unlisten = listen("routines://changed", () => {
      void loadConfig();
    });
    return () => {
      void unlisten.then((dispose) => dispose());
    };
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

  async function setLanguage(language: Language) {
    try {
      setConfig(await setLanguageInStore(language));
      setError(null);
    } catch (cause) {
      setError(String(cause));
    }
  }

  return {
    config,
    error,
    saveRoutine,
    deleteRoutine,
    setActiveRoutine,
    setLanguage,
  };
};

const findNewRoutine = (
  before: RoutineConfig | null,
  after: RoutineConfig,
): Routine | null => {
  const knownIds = new Set(before?.routines.map((r) => r.id) ?? []);
  return after.routines.find((r) => !knownIds.has(r.id)) ?? null;
};
