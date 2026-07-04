import { useEffect, useState } from "react";

import { useT } from "../../../shared/i18n/LanguageContext";
import type { MessageKey } from "../../../shared/i18n/messages";
import { useArmedConfirm } from "../../../shared/useArmedConfirm";
import { checkAccessibilityPermission, fetchInstalledApps } from "../api";
import {
  clampActionsToPreset,
  derivePreset,
  hasAnyRegion,
  presetLabelKey,
  PRESET_REGIONS,
  regionLabelKey,
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
/// Placement works directly on the monitor: select an action card, then
/// click a region on the monitor to place it there.
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
  const [selectedIndex, setSelectedIndex] = useState<number | null>(
    routine.actions.length > 0 ? 0 : null,
  );
  const [validationError, setValidationError] = useState<MessageKey | null>(
    null,
  );
  const [savedFlash, setSavedFlash] = useState(false);
  const [installedApps, setInstalledApps] = useState<string[]>([]);

  useEffect(() => {
    let cancelled = false;
    async function loadInstalledApps() {
      const apps = await fetchInstalledApps();
      if (!cancelled) {
        setInstalledApps(apps);
      }
    }
    void loadInstalledApps();
    return () => {
      cancelled = true;
    };
  }, []);

  const isDirty = JSON.stringify(draft) !== JSON.stringify(routine);

  const updateActions = (actions: Action[]) => {
    setDraft({ ...draft, actions });
  };

  const handlePresetChange = (next: LayoutPreset) => {
    setPreset(next);
    setDraft({ ...draft, actions: clampActionsToPreset(draft.actions, next) });
  };

  const handleRegionClick = (region: Region) => {
    if (selectedIndex === null) {
      return;
    }
    const current = draft.actions[selectedIndex];
    if (!current) {
      return;
    }
    const nextRegion = (current.region ?? null) === region ? null : region;
    updateActions(
      draft.actions.map((action, index) =>
        index === selectedIndex ? { ...action, region: nextRegion } : action,
      ),
    );
  };

  const handleClearRegion = (index: number) => {
    updateActions(
      draft.actions.map((action, i) =>
        i === index ? { ...action, region: null } : action,
      ),
    );
  };

  const handleAddAction = (kind: ActionKind) => {
    const next = [...draft.actions, buildAction(kind, "")];
    updateActions(next);
    setSelectedIndex(next.length - 1);
  };

  const handleRemoveAction = (index: number) => {
    updateActions(draft.actions.filter((_, i) => i !== index));
    setSelectedIndex(null);
  };

  const handleMoveAction = (index: number, direction: -1 | 1) => {
    const moved = moveAction(draft.actions, index, direction);
    if (moved !== draft.actions) {
      updateActions(moved);
      setSelectedIndex(index + direction);
    }
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

      <div className="canvas">
        <div className="segmented presetBar" role="group" aria-label="Layout preset">
          {(Object.keys(PRESET_REGIONS) as LayoutPreset[]).map((option) => (
            <button
              key={option}
              type="button"
              className={
                option === preset ? "segmentedItem on" : "segmentedItem"
              }
              onClick={() => handlePresetChange(option)}
            >
              {t(presetLabelKey(option))}
            </button>
          ))}
        </div>
        <MonitorMockup
          preset={preset}
          actions={draft.actions}
          selectedIndex={selectedIndex}
          onRegionClick={handleRegionClick}
        />
        <p className="canvasGuide">{t("editor.layoutHint")}</p>
      </div>
      <PlacementPermissionHint needed={hasAnyRegion(draft.actions)} />

      <h2 className="editorSectionTitle">{t("editor.actions")}</h2>
      <p className="editorHint">{t("editor.actionsHint")}</p>

      <datalist id="installed-apps">
        {installedApps.map((app) => (
          <option key={app} value={app} />
        ))}
      </datalist>

      <div className="actionCards">
        {draft.actions.map((action, index) => (
          <ActionCard
            key={index}
            action={action}
            index={index}
            total={draft.actions.length}
            isSelected={index === selectedIndex}
            onSelect={() => setSelectedIndex(index)}
            onChange={(next) =>
              updateActions(
                draft.actions.map((a, i) => (i === index ? next : a)),
              )
            }
            onClearRegion={() => handleClearRegion(index)}
            onMove={(direction) => handleMoveAction(index, direction)}
            onRemove={() => handleRemoveAction(index)}
          />
        ))}
      </div>

      <div className="addActionRow">
        <button
          type="button"
          className="ghostButton"
          onClick={() => handleAddAction("open-app")}
        >
          {t("editor.addApp")}
        </button>
        <button
          type="button"
          className="ghostButton"
          onClick={() => handleAddAction("open-url")}
        >
          {t("editor.addUrl")}
        </button>
      </div>

      {validationError && <p className="editorError">{t(validationError)}</p>}

      <footer className="editorFooter">
        <DeleteRoutineButton onDelete={() => onDelete(routine.id)} />
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

/// Deleting is two-step: the first click arms the button, the second
/// within a few seconds actually deletes.
const DeleteRoutineButton = ({ onDelete }: { onDelete: () => void }) => {
  const t = useT();
  const { armed, trigger } = useArmedConfirm(onDelete);

  return (
    <button
      type="button"
      className={armed ? "dangerButton armed" : "dangerButton"}
      onClick={trigger}
    >
      {armed ? t("delete.confirm") : t("editor.delete")}
    </button>
  );
};

const ActiveToggle = ({
  isActive,
  onToggle,
}: {
  isActive: boolean;
  onToggle: () => Promise<void>;
}) => {
  const t = useT();
  return (
    <button
      type="button"
      className={isActive ? "activeToggle on" : "activeToggle"}
      onClick={onToggle}
    >
      <span className="activeToggleDot" />
      {isActive ? t("editor.active") : t("editor.setActive")}
    </button>
  );
};

interface MonitorMockupProps {
  preset: LayoutPreset;
  actions: Action[];
  selectedIndex: number | null;
  onRegionClick: (region: Region) => void;
}

/// Interactive miniature display: every region (and the full-screen strip
/// below it) is a click target that places the selected action.
const MonitorMockup = ({
  preset,
  actions,
  selectedIndex,
  onRegionClick,
}: MonitorMockupProps) => {
  const t = useT();
  const indexed = actions.map((action, index) => ({ action, index }));
  const fullScreen = indexed.filter(({ action }) => action.region === "full");

  return (
    <div className="monitor">
      <div className={`monitorScreen preset-${preset}`}>
        {PRESET_REGIONS[preset].map((region) => {
          const assigned = indexed.filter(
            ({ action }) => action.region === region,
          );
          const className =
            assigned.length > 0 ? "monitorCell filled" : "monitorCell";
          return (
            <button
              key={region}
              type="button"
              className={className}
              onClick={() => onRegionClick(region)}
            >
              {assigned.length > 0 ? (
                assigned.map(({ action, index }) => (
                  <span
                    key={index}
                    className={
                      index === selectedIndex
                        ? "monitorChip selected"
                        : "monitorChip"
                    }
                  >
                    {actionLabel(action)}
                  </span>
                ))
              ) : (
                <span className="monitorCellHint">
                  {t(regionLabelKey(region))}
                </span>
              )}
            </button>
          );
        })}
      </div>
      <div className="monitorStand" />
      <button
        type="button"
        className={fullScreen.length > 0 ? "fullTarget filled" : "fullTarget"}
        onClick={() => onRegionClick("full")}
      >
        <span className="fullTargetLabel">{t("region.full")}</span>
        {fullScreen.map(({ action, index }) => (
          <span
            key={index}
            className={
              index === selectedIndex ? "monitorChip selected" : "monitorChip"
            }
          >
            {actionLabel(action)}
          </span>
        ))}
      </button>
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

interface ActionCardProps {
  action: Action;
  index: number;
  total: number;
  isSelected: boolean;
  onSelect: () => void;
  onChange: (action: Action) => void;
  onClearRegion: () => void;
  onMove: (direction: -1 | 1) => void;
  onRemove: () => void;
}

const ActionCard = ({
  action,
  index,
  total,
  isSelected,
  onSelect,
  onChange,
  onClearRegion,
  onMove,
  onRemove,
}: ActionCardProps) => {
  const t = useT();
  const className = isSelected ? "actionCard selected" : "actionCard";

  return (
    <div className={className} onClick={onSelect}>
      <span className="actionIndex">{index + 1}</span>
      <select
        className="actionKindSelect"
        value={action.type}
        aria-label="Action kind"
        onChange={(event) =>
          onChange(
            buildAction(
              event.target.value as ActionKind,
              actionValue(action),
              action.region ?? null,
            ),
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
        list={action.type === "open-app" ? "installed-apps" : undefined}
        onChange={(event) =>
          onChange(
            buildAction(action.type, event.target.value, action.region ?? null),
          )
        }
      />
      {action.region ? (
        <button
          type="button"
          className="regionBadge"
          title={t("editor.noPlacement")}
          onClick={(event) => {
            event.stopPropagation();
            onClearRegion();
          }}
        >
          {t(regionLabelKey(action.region))} ✕
        </button>
      ) : (
        <span className="regionBadge none">{t("editor.noPlacement")}</span>
      )}
      <div className="actionRowControls">
        <button
          type="button"
          className="iconButton"
          disabled={index === 0}
          onClick={(event) => {
            event.stopPropagation();
            onMove(-1);
          }}
          aria-label="Move up"
        >
          ↑
        </button>
        <button
          type="button"
          className="iconButton"
          disabled={index === total - 1}
          onClick={(event) => {
            event.stopPropagation();
            onMove(1);
          }}
          aria-label="Move down"
        >
          ↓
        </button>
        <button
          type="button"
          className="iconButton remove"
          onClick={(event) => {
            event.stopPropagation();
            onRemove();
          }}
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
