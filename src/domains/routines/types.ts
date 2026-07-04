export type Region =
  | "full"
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
  | { type: "open-app"; name: string; region?: Region | null }
  | { type: "open-url"; url: string; region?: Region | null };

export type ActionKind = Action["type"];

export interface Routine {
  id: string;
  name: string;
  actions: Action[];
}

import type { Language } from "../../shared/i18n/messages";
import type { ThemeSetting } from "../../shared/theme";

export interface RoutineConfig {
  activeRoutineId: string | null;
  routines: Routine[];
  /// null until the user picks a language in first-launch onboarding.
  language: Language | null;
  theme: ThemeSetting;
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
  return action.type === "open-app" ? action.name : action.url;
};

export const actionLabel = (action: Action): string => {
  return actionValue(action) || (action.type === "open-app" ? "app" : "url");
};

export const buildAction = (
  kind: ActionKind,
  value: string,
  region: Region | null = null,
): Action => {
  return kind === "open-app"
    ? { type: "open-app", name: value, region }
    : { type: "open-url", url: value, region };
};
