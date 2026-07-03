export type Action =
  | { type: "open-app"; name: string }
  | { type: "open-url"; url: string };

export type ActionKind = Action["type"];

export interface Routine {
  id: string;
  name: string;
  actions: Action[];
}

export interface RoutineConfig {
  activeRoutineId: string | null;
  routines: Routine[];
}

export const actionValue = (action: Action): string => {
  return action.type === "open-app" ? action.name : action.url;
};

export const buildAction = (kind: ActionKind, value: string): Action => {
  return kind === "open-app"
    ? { type: "open-app", name: value }
    : { type: "open-url", url: value };
};
