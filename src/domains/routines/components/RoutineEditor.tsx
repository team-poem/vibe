import { useEffect, useState } from "react";

import { checkAccessibilityPermission } from "../api";
import {
  clampActionsToPreset,
  derivePreset,
  hasAnyRegion,
  PRESET_LABELS,
  PRESET_REGIONS,
  REGION_LABELS,
  selectableRegions,
} from "../layout";
import type { LayoutPreset } from "../layout";
import { actionLabel, actionValue, buildAction } from "../types";
import type { Action, ActionKind, Region, Routine } from "../types";

type ValidationResult = { ok: true } | { ok: false; reason: string };

const checkIsDraftValid = (draft: Routine): ValidationResult => {
  if (draft.name.trim() === "") {
    return { ok: false, reason: "Routine name cannot be empty." };
  }
  if (draft.actions.length === 0) {
    return { ok: false, reason: "Add at least one action." };
  }
  for (const action of draft.actions) {
    if (actionValue(action).trim() === "") {
      return { ok: false, reason: "Every action needs a value." };
    }
    const isInvalidUrl =
      action.type === "open-url" && !/^https?:\/\//.test(action.url);
    if (isInvalidUrl) {
      return { ok: false, reason: "URLs must start with http:// or https://." };
    }
  }
  return { ok: true };
};

interface RoutineEditorProps {
  routine: Routine;
  isActive: boolean;
  onSave: (routine: Routine) => Promise<void>;
  onDelete: (id: string) => Promise<void>;
  onActivate: (id: string | null) => Promise<void>;
}

/// Edits one routine as a local draft; nothing touches the store until Save.
/// Mount with `key={routine.id}` so switching routines resets the draft.
export const RoutineEditor = ({
  routine,
  isActive,
  onSave,
  onDelete,
  onActivate,
}: RoutineEditorProps) => {
  const [draft, setDraft] = useState<Routine>(routine);
  const [preset, setPreset] = useState<LayoutPreset>(() =>
    derivePreset(routine.actions),
  );
  const [validationError, setValidationError] = useState<string | null>(null);
  const [savedFlash, setSavedFlash] = useState(false);

  const isDirty = JSON.stringify(draft) !== JSON.stringify(routine);

  const updateActions = (actions: Action[]) => {
    setDraft({ ...draft, actions });
  };

  const handlePresetChange = (next: LayoutPreset) => {
    setPreset(next);
    setDraft({ ...draft, actions: clampActionsToPreset(draft.actions, next) });
  };

  async function handleSave() {
    const validation = checkIsDraftValid(draft);
    if (!validation.ok) {
      setValidationError(validation.reason);
      return;
    }
    setValidationError(null);
    await onSave(draft);
    setSavedFlash(true);
    window.setTimeout(() => setSavedFlash(false), SAVED_FLASH_MS);
  }

  async function handleActivateToggle() {
    await onActivate(isActive ? null : routine.id);
  }

  return (
    <section className="editor">
      <div className="editorHeader">
        <input
          className="routineNameInput"
          value={draft.name}
          onChange={(event) => setDraft({ ...draft, name: event.target.value })}
          placeholder="Routine name"
          aria-label="Routine name"
        />
        <ActiveToggle isActive={isActive} onToggle={handleActivateToggle} />
      </div>

      <h2 className="editorSectionTitle">Layout</h2>
      <p className="editorHint">
        Pick a split, then assign each action to a screen region.
      </p>
      <div className="layoutSection">
        <div className="presetGroup" role="group" aria-label="Layout preset">
          {(Object.keys(PRESET_REGIONS) as LayoutPreset[]).map((option) => (
            <button
              key={option}
              type="button"
              className={option === preset ? "presetButton on" : "presetButton"}
              onClick={() => handlePresetChange(option)}
            >
              {PRESET_LABELS[option]}
            </button>
          ))}
        </div>
        <MonitorMockup preset={preset} actions={draft.actions} />
      </div>
      <PlacementPermissionHint needed={hasAnyRegion(draft.actions)} />

      <h2 className="editorSectionTitle">Actions</h2>
      <p className="editorHint">Run in order when you clap twice.</p>

      <div className="actionRows">
        {draft.actions.map((action, index) => (
          <ActionRow
            key={index}
            action={action}
            index={index}
            total={draft.actions.length}
            regionOptions={selectableRegions(preset)}
            onChange={(next) =>
              updateActions(draft.actions.map((a, i) => (i === index ? next : a)))
            }
            onMove={(direction) =>
              updateActions(moveAction(draft.actions, index, direction))
            }
            onRemove={() =>
              updateActions(draft.actions.filter((_, i) => i !== index))
            }
          />
        ))}
      </div>

      <div className="addActionRow">
        <button
          type="button"
          className="ghostButton"
          onClick={() =>
            updateActions([...draft.actions, buildAction("open-app", "")])
          }
        >
          + Launch app
        </button>
        <button
          type="button"
          className="ghostButton"
          onClick={() =>
            updateActions([...draft.actions, buildAction("open-url", "")])
          }
        >
          + Open URL
        </button>
      </div>

      {validationError && <p className="editorError">{validationError}</p>}

      <footer className="editorFooter">
        <button
          type="button"
          className="dangerButton"
          onClick={() => onDelete(routine.id)}
        >
          Delete
        </button>
        <button
          type="button"
          className="primaryButton"
          onClick={handleSave}
          disabled={!isDirty && !savedFlash}
        >
          {savedFlash ? "Saved" : "Save"}
        </button>
      </footer>
    </section>
  );
};

const SAVED_FLASH_MS = 1500;

const ActiveToggle = ({
  isActive,
  onToggle,
}: {
  isActive: boolean;
  onToggle: () => Promise<void>;
}) => {
  return (
    <button
      type="button"
      className={isActive ? "activeToggle on" : "activeToggle"}
      onClick={onToggle}
    >
      <span className="activeToggleDot" />
      {isActive ? "Active" : "Set active"}
    </button>
  );
};

interface MonitorMockupProps {
  preset: LayoutPreset;
  actions: Action[];
}

/// Miniature display, like the monitor in macOS display settings, showing
/// which action lands in which region of the chosen split.
const MonitorMockup = ({ preset, actions }: MonitorMockupProps) => {
  const fullScreenActions = actions.filter((a) => a.region === "full");

  return (
    <div className="monitor">
      <div className={`monitorScreen preset-${preset}`}>
        {PRESET_REGIONS[preset].map((region) => {
          const assigned = actions.filter((a) => a.region === region);
          const className =
            assigned.length > 0 ? "monitorCell filled" : "monitorCell";
          return (
            <div key={region} className={className}>
              {assigned.length > 0 ? (
                assigned.map((action, i) => (
                  <span key={i} className="monitorCellApp">
                    {actionLabel(action)}
                  </span>
                ))
              ) : (
                <span className="monitorCellHint">{REGION_LABELS[region]}</span>
              )}
            </div>
          );
        })}
      </div>
      <div className="monitorStand" />
      {fullScreenActions.length > 0 && (
        <p className="monitorFullNote">
          Full screen: {fullScreenActions.map(actionLabel).join(", ")}
        </p>
      )}
    </div>
  );
};

/// Shown only when regions are assigned but macOS has not granted the
/// Accessibility permission yet.
const PlacementPermissionHint = ({ needed }: { needed: boolean }) => {
  const [granted, setGranted] = useState<boolean | null>(null);

  useEffect(() => {
    if (!needed) {
      return;
    }
    let cancelled = false;
    async function checkQuietly() {
      const ok = await checkAccessibilityPermission(false);
      if (!cancelled) {
        setGranted(ok);
      }
    }
    void checkQuietly();
    return () => {
      cancelled = true;
    };
  }, [needed]);

  if (!needed || granted !== false) {
    return null;
  }

  async function handleEnableClick() {
    setGranted(await checkAccessibilityPermission(true));
  }

  return (
    <div className="permissionHint">
      <span>
        Window placement needs the Accessibility permission — enable V.I.B.E in
        System Settings.
      </span>
      <button type="button" className="ghostButton" onClick={handleEnableClick}>
        Enable…
      </button>
    </div>
  );
};

const ACTION_KIND_LABELS: Record<ActionKind, string> = {
  "open-app": "Launch app",
  "open-url": "Open URL",
};

const ACTION_PLACEHOLDERS: Record<ActionKind, string> = {
  "open-app": "App name, e.g. Cursor",
  "open-url": "https://…",
};

interface ActionRowProps {
  action: Action;
  index: number;
  total: number;
  regionOptions: Region[];
  onChange: (action: Action) => void;
  onMove: (direction: -1 | 1) => void;
  onRemove: () => void;
}

const ActionRow = ({
  action,
  index,
  total,
  regionOptions,
  onChange,
  onMove,
  onRemove,
}: ActionRowProps) => {
  return (
    <div className="actionRow">
      <span className="actionIndex">{index + 1}</span>
      <select
        className="actionKindSelect"
        value={action.type}
        aria-label="Action kind"
        onChange={(event) =>
          onChange(
            buildAction(event.target.value as ActionKind, actionValue(action)),
          )
        }
      >
        {Object.entries(ACTION_KIND_LABELS).map(([kind, label]) => (
          <option key={kind} value={kind}>
            {label}
          </option>
        ))}
      </select>
      <input
        className="actionValueInput"
        value={actionValue(action)}
        placeholder={ACTION_PLACEHOLDERS[action.type]}
        aria-label="Action value"
        onChange={(event) =>
          onChange(
            buildAction(action.type, event.target.value, action.region ?? null),
          )
        }
      />
      <select
        className="actionRegionSelect"
        value={action.region ?? ""}
        aria-label="Screen region"
        onChange={(event) =>
          onChange({
            ...action,
            region: (event.target.value || null) as Region | null,
          })
        }
      >
        <option value="">No placement</option>
        {regionOptions.map((region) => (
          <option key={region} value={region}>
            {REGION_LABELS[region]}
          </option>
        ))}
      </select>
      <div className="actionRowControls">
        <button
          type="button"
          className="iconButton"
          disabled={index === 0}
          onClick={() => onMove(-1)}
          aria-label="Move up"
        >
          ↑
        </button>
        <button
          type="button"
          className="iconButton"
          disabled={index === total - 1}
          onClick={() => onMove(1)}
          aria-label="Move down"
        >
          ↓
        </button>
        <button
          type="button"
          className="iconButton remove"
          onClick={onRemove}
          aria-label="Remove action"
        >
          ✕
        </button>
      </div>
    </div>
  );
};

const moveAction = (actions: Action[], index: number, direction: -1 | 1) => {
  const target = index + direction;
  if (target < 0 || target >= actions.length) {
    return actions;
  }
  const next = [...actions];
  [next[index], next[target]] = [next[target], next[index]];
  return next;
};
