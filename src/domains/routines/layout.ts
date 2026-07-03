import type { Action, Region } from "./types";

/// Split layouts the monitor mockup can show. The preset is a pure UI
/// concept: only each action's `region` is persisted.
export type LayoutPreset = "halves" | "thirds" | "quarters";

export const PRESET_REGIONS: Record<LayoutPreset, Region[]> = {
  halves: ["left-half", "right-half"],
  thirds: ["left-third", "center-third", "right-third"],
  quarters: ["top-left", "top-right", "bottom-left", "bottom-right"],
};

export const PRESET_LABELS: Record<LayoutPreset, string> = {
  halves: "2 split",
  thirds: "3 split",
  quarters: "4 split",
};

export const REGION_LABELS: Record<Region, string> = {
  full: "Full screen",
  "left-half": "Left half",
  "right-half": "Right half",
  "left-third": "Left third",
  "center-third": "Center third",
  "right-third": "Right third",
  "top-left": "Top left",
  "top-right": "Top right",
  "bottom-left": "Bottom left",
  "bottom-right": "Bottom right",
};

export const derivePreset = (actions: Action[]): LayoutPreset => {
  const regions = actions
    .map((action) => action.region)
    .filter((region): region is Region => Boolean(region));

  if (regions.some((region) => PRESET_REGIONS.thirds.includes(region))) {
    return "thirds";
  }
  if (regions.some((region) => PRESET_REGIONS.quarters.includes(region))) {
    return "quarters";
  }
  return "halves";
};

/// Regions offered to an action under the given preset.
export const selectableRegions = (preset: LayoutPreset): Region[] => {
  return [...PRESET_REGIONS[preset], "full"];
};

/// Clear regions that no longer exist after a preset switch ("full" always
/// survives).
export const clampActionsToPreset = (
  actions: Action[],
  preset: LayoutPreset,
): Action[] => {
  const allowed = new Set<Region>(selectableRegions(preset));
  return actions.map((action) =>
    action.region && !allowed.has(action.region)
      ? { ...action, region: null }
      : action,
  );
};

export const hasAnyRegion = (actions: Action[]): boolean => {
  return actions.some((action) => Boolean(action.region));
};
