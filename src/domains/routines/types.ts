import type { Language } from "../../shared/i18n/messages";
import type { ThemeSetting } from "../../shared/theme";

export type Region =
  | "full"
  | "centered"
  | "left-half"
  | "right-half"
  | "left-third"
  | "center-third"
  | "right-third"
  | "top-left"
  | "top-right"
  | "bottom-left"
  | "bottom-right";

export type Action =
  | {
      type: "open-app";
      name: string;
      region?: Region | null;
      display?: number | null;
    }
  | {
      type: "open-url";
      url: string;
      region?: Region | null;
      display?: number | null;
    }
  | {
      type: "open-file";
      path: string;
      region?: Region | null;
      display?: number | null;
    };

export type ActionKind = Action["type"];

export interface Routine {
  id: string;
  name: string;
  actions: Action[];
}

export interface DisplayInfo {
  id: number;
  x: number;
  y: number;
  width: number;
  height: number;
  isMain: boolean;
}

export type ClapSensitivity = "low" | "medium" | "high";

export interface RoutineConfig {
  activeRoutineId: string | null;
  routines: Routine[];
  /// null until the user picks a language in first-launch onboarding.
  language: Language | null;
  theme: ThemeSetting;
  sensitivity: ClapSensitivity;
}

export interface ActionOutcome {
  label: string;
  success: boolean;
  detail: string;
}

export interface ExecutionRecord {
  atEpochMs: number;
  routineName: string;
  success: boolean;
  outcomes: ActionOutcome[];
}

export const actionValue = (action: Action): string => {
  switch (action.type) {
    case "open-app":
      return action.name;
    case "open-url":
      return action.url;
    case "open-file":
      return action.path;
  }
};

export const actionLabel = (action: Action): string => {
  if (action.type === "open-file") {
    return action.path.split("/").pop() || "file";
  }
  if (action.type === "open-url") {
    try {
      return new URL(action.url).hostname.replace(/^www\./, "");
    } catch {
      return action.url || "url";
    }
  }
  return action.name || "app";
};

export const buildAction = (
  kind: ActionKind,
  value: string,
  region: Region | null = null,
  display: number | null = null,
): Action => {
  const base: Action =
    kind === "open-app"
      ? { type: "open-app", name: value, region }
      : kind === "open-url"
        ? { type: "open-url", url: value, region }
        : { type: "open-file", path: value, region };
  if (display !== null) {
    base.display = display;
  }
  return base;
};
