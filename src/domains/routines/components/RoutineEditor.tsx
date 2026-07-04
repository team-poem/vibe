import { useEffect, useState } from "react";

import { useT } from "../../../shared/i18n/LanguageContext";
import type { MessageKey } from "../../../shared/i18n/messages";
import { checkAccessibilityPermission } from "../api";
import {
  clampActionsToPreset,
  derivePreset,
  hasAnyRegion,
  presetLabelKey,
  PRESET_REGIONS,
  regionLabelKey,
  selectableRegions,
} from "../layout";
import type { LayoutPreset } from "../layout";
import { actionLabel, actionValue, buildAction } from "../types";
import type { Action, ActionKind, Region, Routine } from "../types";

type ValidationResult = { ok: true } | { ok: false; reason: MessageKey };

const checkIsDraftValid = (draft: Routine): ValidationResult => {
  if (draft.name.trim() === "") {
    return { ok: false, reason: "validation.nameEmpty" };
  }
  if (draft.actions.length === 0) {
    return { ok: false, reason: "validation.actionsEmpty" };
  }
  for (const action of draft.actions) {
    if (actionValue(action).trim() === "") {
      return { ok: false, reason: "validation.valueEmpty" };
    }
    const isInvalidUrl =
      action.type === "open-url" && !/^https?:\/\//.test(action.url);
    if (isInvalidUrl) {
      return { ok: false, reason: "validation.urlInvalid" };
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
  const t = useT();
  const [draft, setDraft] = useState<Routine>(routine);
  const [preset, setPreset] = useState<LayoutPreset>(() =>
    derivePreset(routine.actions),
  );
  const [validationError, setValidationError] = useState<MessageKey | null>(
    null,
  );
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
          placeholder={t("editor.namePlaceholder")}
          aria-label="Routine name"
        />
        <ActiveToggle isActive={isActive} onToggle={handleActivateToggle} />
      </div>

      <h2 className="editorSectionTitle">{t("editor.layout")}</h2>
      <p className="editorHint">{t("editor.layoutHint")}</p>
      <div className="layoutSection">
        <div className="presetGroup" role="group" aria-label="Layout preset">
          {(Object.keys(PRESET_REGIONS) as LayoutPreset[]).map((option) => (
            <button
              key={option}
              type="button"
              className={option === preset ? "presetButton on" : "presetButton"}
              onClick={() => handlePresetChange(option)}
            >
              {t(presetLabelKey(option))}
            </button>
          ))}
        </div>
        <MonitorMockup preset={preset} actions={draft.actions} />
      </div>
      <PlacementPermissionHint needed={hasAnyRegion(draft.actions)} />

      <h2 className="editorSectionTitle">{t("editor.actions")}</h2>
      <p className="editorHint">{t("editor.actionsHint")}</p>

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
          {t("editor.addApp")}
        </button>
        <button
          type="button"
          className="ghostButton"
          onClick={() =>
            updateActions([...draft.actions, buildAction("open-url", "")])
          }
        >
          {t("editor.addUrl")}
        </button>
      </div>

      {validationError && <p className="editorError">{t(validationError)}</p>}

      <footer className="editorFooter">
        <button
          type="button"
          className="dangerButton"
          onClick={() => onDelete(routine.id)}
        >
          {t("editor.delete")}
        </button>
        <button
          type="button"
          className="primaryButton"
          onClick={handleSave}
          disabled={!isDirty && !savedFlash}
        >
          {savedFlash ? t("editor.saved") : t("editor.save")}
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
  const t = useT();
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
                <span className="monitorCellHint">
                  {t(regionLabelKey(region))}
                </span>
              )}
            </div>
          );
        })}
      </div>
      <div className="monitorStand" />
      {fullScreenActions.length > 0 && (
        <p className="monitorFullNote">
          {t("editor.fullNote")} {fullScreenActions.map(actionLabel).join(", ")}
        </p>
      )}
    </div>
  );
};

/// Shown only when regions are assigned but macOS has not granted the
/// Accessibility permission yet.
const PlacementPermissionHint = ({ needed }: { needed: boolean }) => {
  const t = useT();
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
      <span>{t("permission.hint")}</span>
      <button type="button" className="ghostButton" onClick={handleEnableClick}>
        {t("permission.enable")}
      </button>
    </div>
  );
};

const ACTION_KINDS: ActionKind[] = ["open-app", "open-url"];

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
  const t = useT();
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
        {ACTION_KINDS.map((kind) => (
          <option key={kind} value={kind}>
            {t(`action.kind.${kind}`)}
          </option>
        ))}
      </select>
      <input
        className="actionValueInput"
        value={actionValue(action)}
        placeholder={t(`action.placeholder.${action.type}`)}
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
        <option value="">{t("editor.noPlacement")}</option>
        {regionOptions.map((region) => (
          <option key={region} value={region}>
            {t(regionLabelKey(region))}
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
