import type { MessageKey } from "../../shared/i18n/messages";
import type { Action, Region } from "./types";

/// Split layouts the monitor mockup can show. The preset is a pure UI
/// concept: only each action's `region` is persisted.
export type LayoutPreset = "single" | "halves" | "thirds" | "quarters";

export const PRESET_REGIONS: Record<LayoutPreset, Region[]> = {
  single: ["full"],
  halves: ["left-half", "right-half"],
  thirds: ["left-third", "center-third", "right-third"],
  quarters: ["top-left", "top-right", "bottom-left", "bottom-right"],
};

export const presetLabelKey = (preset: LayoutPreset): MessageKey => {
  return `preset.${preset}`;
};

export const regionLabelKey = (region: Region): MessageKey => {
  return `region.${region}`;
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
  if (regions.some((region) => PRESET_REGIONS.halves.includes(region))) {
    return "halves";
  }
  if (regions.includes("full")) {
    return "single";
  }
  return "halves";
};

export const hasAnyRegion = (actions: Action[]): boolean => {
  return actions.some((action) => Boolean(action.region));
};
