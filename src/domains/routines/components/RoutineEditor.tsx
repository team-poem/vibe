import { useEffect, useRef, useState } from "react";

import { open as openFileDialog } from "@tauri-apps/plugin-dialog";

import { useT } from "../../../shared/i18n/LanguageContext";
import { useArmedConfirm } from "../../../shared/useArmedConfirm";
import {
  checkAccessibilityPermission,
  fetchDisplays,
  fetchInstalledApps,
  repairAccessibilityPermission,
  restartApp,
} from "../api";
import {
  derivePreset,
  hasAnyRegion,
  presetLabelKey,
  PRESET_REGIONS,
  regionLabelKey,
} from "../layout";
import type { LayoutPreset } from "../layout";
import { actionLabel, actionValue, buildAction } from "../types";
import type { Action, ActionKind, DisplayInfo, Region, Routine } from "../types";

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
  const [selectedIndex, setSelectedIndex] = useState<number | null>(
    routine.actions.length > 0 ? 0 : null,
  );
  const [savedFlash, setSavedFlash] = useState(false);
  const [installedApps, setInstalledApps] = useState<string[]>([]);
  const [displays, setDisplays] = useState<DisplayInfo[]>([]);

  useEffect(() => {
    let cancelled = false;
    async function loadEditorContext() {
      const [apps, connectedDisplays] = await Promise.all([
        fetchInstalledApps(),
        fetchDisplays(),
      ]);
      if (!cancelled) {
        setInstalledApps(apps);
        setDisplays(connectedDisplays);
      }
    }
    async function refreshDisplays() {
      const connectedDisplays = await fetchDisplays();
      if (!cancelled) {
        setDisplays(connectedDisplays);
      }
    }
    void loadEditorContext();
    // Displays hot-plug at any time; keep the picker in sync.
    const timer = window.setInterval(refreshDisplays, DISPLAY_REFRESH_MS);
    window.addEventListener("focus", refreshDisplays);
    return () => {
      cancelled = true;
      window.clearInterval(timer);
      window.removeEventListener("focus", refreshDisplays);
    };
  }, []);

  // Which display's canvas is being edited — pure view state; each
  // action stores its own (display, region) target.
  const [viewDisplayId, setViewDisplayId] = useState<number | null>(null);

  const mainDisplayId = displays.find((d) => d.isMain)?.id ?? null;
  const viewedDisplay =
    displays.find((d) => d.id === viewDisplayId) ??
    displays.find((d) => d.isMain) ??
    displays[0] ??
    null;

  const targetDisplayOf = (action: Action): number | null =>
    action.display ?? mainDisplayId;

  const placedOnViewedDisplay = draft.actions
    .map((action, index) => ({ action, index }))
    .filter(
      ({ action }) =>
        Boolean(action.region) &&
        (displays.length < 2 ||
          targetDisplayOf(action) === (viewedDisplay?.id ?? mainDisplayId)),
    );

  // The split tab follows the viewed display: derived from what is placed
  // there, and manual choices are remembered per display.
  const [presetOverrides, setPresetOverrides] = useState<
    Record<string, LayoutPreset>
  >({});
  const displayKey = String(viewedDisplay?.id ?? "main");
  const preset =
    presetOverrides[displayKey] ??
    derivePreset(placedOnViewedDisplay.map(({ action }) => action));

  const groupKeyOf = (action: Action): string =>
    action.region
      ? `${targetDisplayOf(action) ?? "m"}:${action.region}`
      : "unplaced";

  // Actions grouped by their (display, region) stack; order inside a group
  // is the window stacking order (top = frontmost).
  const actionGroups = (() => {
    type Group = {
      key: string;
      label: string;
      sortKey: number;
      displayId: number | null;
      items: { action: Action; index: number }[];
    };
    const list: Group[] = [];
    const viewedId = viewedDisplay?.id ?? mainDisplayId;
    draft.actions.forEach((action, index) => {
      // The tab bar scopes the list: only this display's stacks are shown
      // (unplaced actions always are).
      if (
        action.region &&
        displays.length > 1 &&
        targetDisplayOf(action) !== viewedId
      ) {
        return;
      }
      const key = groupKeyOf(action);
      let group = list.find((g) => g.key === key);
      if (!group) {
        if (action.region) {
          group = {
            key,
            label: t(regionLabelKey(action.region)),
            sortKey: REGION_ORDER.indexOf(action.region),
            displayId: targetDisplayOf(action),
            items: [],
          };
        } else {
          group = {
            key,
            label: t("editor.noPlacement"),
            sortKey: Number.MAX_SAFE_INTEGER,
            displayId: null,
            items: [],
          };
        }
        list.push(group);
      }
      group.items.push({ action, index });
    });
    list.sort((a, b) => a.sortKey - b.sortKey);
    return list;
  })();

  const withTarget = (action: Action, region: Region | null): Action => {
    const next = { ...action, region };
    if (region !== null && viewedDisplay && !viewedDisplay.isMain) {
      next.display = viewedDisplay.id;
    } else {
      delete next.display;
    }
    return next;
  };

  const isDirty = JSON.stringify(draft) !== JSON.stringify(routine);

  // Autosave: changes persist a moment after the last edit. The preset is
  // a view; placements outside it are kept, not cleared.
  useEffect(() => {
    if (!isDirty || draft.name.trim() === "") {
      return;
    }
    const timer = window.setTimeout(() => {
      void onSave(draft).then(() => {
        setSavedFlash(true);
        window.setTimeout(() => setSavedFlash(false), SAVED_FLASH_MS);
      });
    }, AUTOSAVE_DELAY_MS);
    return () => window.clearTimeout(timer);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [draft, isDirty]);

  const updateActions = (actions: Action[]) => {
    setDraft({ ...draft, actions });
  };

  const handlePresetChange = (next: LayoutPreset) => {
    setPresetOverrides((prev) => ({ ...prev, [displayKey]: next }));
  };

  const handleSelectAction = (index: number) => {
    setSelectedIndex(index);
    const action = draft.actions[index];
    if (action?.region && displays.length > 1) {
      const target = targetDisplayOf(action);
      if (target !== null && target !== viewedDisplay?.id) {
        setViewDisplayId(target);
      }
    }
  };

  const handleRegionClick = (region: Region) => {
    if (selectedIndex === null) {
      return;
    }
    const current = draft.actions[selectedIndex];
    if (!current) {
      return;
    }
    const alreadyHere =
      (current.region ?? null) === region &&
      targetDisplayOf(current) === (viewedDisplay?.id ?? mainDisplayId);
    const nextRegion = alreadyHere ? null : region;
    updateActions(
      draft.actions.map((action, index) =>
        index === selectedIndex ? withTarget(action, nextRegion) : action,
      ),
    );
  };

  const handleRegionDrop = (region: Region, index: number) => {
    if (!draft.actions[index]) {
      return;
    }
    updateActions(
      draft.actions.map((action, i) =>
        i === index ? withTarget(action, region) : action,
      ),
    );
    setSelectedIndex(index);
  };

  const handleClearRegion = (index: number) => {
    updateActions(
      draft.actions.map((action, i) => {
        if (i !== index) {
          return action;
        }
        const { display: _display, ...rest } = action;
        return { ...rest, region: null } as Action;
      }),
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

  /// Dropping a card onto another card moves it into that card's stack
  /// (same region and display) at that position.
  const handleReorderDrop = (from: number, to: number) => {
    if (from === to || !draft.actions[from] || !draft.actions[to]) {
      return;
    }
    const target = draft.actions[to];
    const next = [...draft.actions];
    const [taken] = next.splice(from, 1);
    const moved = { ...taken, region: target.region ?? null };
    if (target.region && target.display !== undefined) {
      moved.display = target.display;
    } else {
      delete moved.display;
    }
    const insertAt = from < to ? to - 1 : to;
    next.splice(insertAt, 0, moved);
    updateActions(next);
    setSelectedIndex(insertAt);
  };

  /// Swap an action with its neighbor inside the same (display, region)
  /// stack — ordering across different regions is meaningless.
  const handleMoveAction = (index: number, direction: -1 | 1) => {
    const current = draft.actions[index];
    if (!current) {
      return;
    }
    const key = groupKeyOf(current);
    const siblings = draft.actions
      .map((action, i) => ({ action, i }))
      .filter(({ action }) => groupKeyOf(action) === key)
      .map(({ i }) => i);
    const position = siblings.indexOf(index);
    const swapWith = siblings[position + direction];
    if (swapWith === undefined) {
      return;
    }
    const next = [...draft.actions];
    [next[index], next[swapWith]] = [next[swapWith], next[index]];
    updateActions(next);
    setSelectedIndex(swapWith);
  };

  async function handleActivateToggle() {
    await onActivate(isActive ? null : routine.id);
  }

  return (
    <section className="editor routineEditor">
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

      {displays.length > 1 && (
        <div className="displayTabs segmented" role="tablist">
          {displays.map((display, order) => (
            <button
              key={display.id}
              type="button"
              role="tab"
              aria-selected={display.id === viewedDisplay?.id}
              className={
                display.id === viewedDisplay?.id
                  ? "segmentedItem on"
                  : "segmentedItem"
              }
              onClick={() => setViewDisplayId(display.id)}
            >
              {t("editor.display")} {order + 1}
            </button>
          ))}
        </div>
      )}

      <PlacementPermissionHint needed={hasAnyRegion(draft.actions)} />

      <div className="editorBody">
        <div className="canvas">
          {displays.length > 1 && (
            <DisplayArrangement
              displays={displays}
              selectedId={viewedDisplay?.id ?? null}
              onPick={(display) => setViewDisplayId(display.id)}
            />
          )}
          <div
            className="segmented presetBar"
            role="group"
            aria-label="Layout preset"
          >
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
            placed={placedOnViewedDisplay}
            selectedIndex={selectedIndex}
            display={viewedDisplay}
            onRegionClick={handleRegionClick}
            onRegionDrop={handleRegionDrop}
            onRegionClear={handleClearRegion}
            onMoveAction={handleMoveAction}
            onChipSelect={handleSelectAction}
          />
          <p className="canvasGuide">{t("editor.layoutHint")}</p>
        </div>

        <div className="actionsColumn">
          <h2 className="editorSectionTitle">{t("editor.actions")}</h2>
          <p className="editorHint">{t("editor.actionsHint")}</p>

          <datalist id="installed-apps">
            {installedApps.map((app) => (
              <option key={app} value={app} />
            ))}
          </datalist>

          <div className="actionCards">
            {actionGroups.map((group) => (
              <div key={group.key} className="actionGroup">
                <h3 className="actionGroupTitle">
                  {group.label}
                  {group.items.length > 1 && (
                    <span className="actionGroupCount">
                      {group.items.length}
                    </span>
                  )}
                </h3>
                {group.items.map(({ action, index }, stackPos) => (
                  <ActionCard
                    key={index}
                    action={action}
                    index={index}
                    stackPos={stackPos}
                    stackSize={group.items.length}
                    isSelected={index === selectedIndex}
                    onSelect={() => handleSelectAction(index)}
                    onChange={(next) =>
                      updateActions(
                        draft.actions.map((a, i) => (i === index ? next : a)),
                      )
                    }
                    onClearRegion={() => handleClearRegion(index)}
                    onMove={(direction) => handleMoveAction(index, direction)}
                    onRemove={() => handleRemoveAction(index)}
                    onReorderTo={(from) => handleReorderDrop(from, index)}
                  />
                ))}
              </div>
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
            <button
              type="button"
              className="ghostButton"
              onClick={() => handleAddAction("open-file")}
            >
              {t("editor.addFile")}
            </button>
          </div>
        </div>
      </div>

      <footer className="editorFooter">
        <DeleteRoutineButton onDelete={() => onDelete(routine.id)} />
        <span className={savedFlash ? "autosaveNote on" : "autosaveNote"}>
          ✓ {t("editor.saved")}
        </span>
      </footer>
    </section>
  );
};

const SAVED_FLASH_MS = 1400;
const AUTOSAVE_DELAY_MS = 600;
const DISPLAY_REFRESH_MS = 5000;

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

const DRAG_ACTION_MIME = "text/plain";

const REGION_ORDER: Region[] = [
  "full",
  "centered",
  "left-half",
  "right-half",
  "left-third",
  "center-third",
  "right-third",
  "top-left",
  "top-right",
  "bottom-left",
  "bottom-right",
];

interface PlacedAction {
  action: Action;
  index: number;
}

interface MonitorMockupProps {
  preset: LayoutPreset;
  placed: PlacedAction[];
  selectedIndex: number | null;
  display: DisplayInfo | null;
  onRegionClick: (region: Region) => void;
  onRegionDrop: (region: Region, actionIndex: number) => void;
  onRegionClear: (actionIndex: number) => void;
  onMoveAction: (actionIndex: number, direction: -1 | 1) => void;
  onChipSelect: (actionIndex: number) => void;
}

/// Interactive miniature display: regions (and the full-screen strip below)
/// accept both a click on the selected action and a dragged action card.
const MonitorMockup = ({
  preset,
  placed,
  selectedIndex,
  display,
  onRegionClick,
  onRegionDrop,
  onRegionClear,
  onMoveAction,
  onChipSelect,
}: MonitorMockupProps) => {
  const t = useT();
  const [dragOverRegion, setDragOverRegion] = useState<Region | null>(null);

  const dropHandlers = (region: Region) => ({
    onDragOver: (event: React.DragEvent) => {
      event.preventDefault();
      setDragOverRegion(region);
    },
    onDragLeave: () => setDragOverRegion(null),
    onDrop: (event: React.DragEvent) => {
      event.preventDefault();
      setDragOverRegion(null);
      const index = Number(event.dataTransfer.getData(DRAG_ACTION_MIME));
      if (!Number.isNaN(index)) {
        onRegionDrop(region, index);
      }
    },
  });

  const ratio = display ? display.width / display.height : 1.6;

  return (
    <div className="monitor">
      <div
        className={`monitorScreen preset-${preset}`}
        style={
          {
            "--monitor-ratio": display
              ? `${display.width} / ${display.height}`
              : "16 / 10",
            "--monitor-ratio-num": String(ratio),
          } as React.CSSProperties
        }
      >
        {PRESET_REGIONS[preset].map((region) => {
          const assigned = placed.filter(
            ({ action }) => action.region === region,
          );
          const classNames = ["monitorCell"];
          if (assigned.length > 0) {
            classNames.push("filled");
          }
          if (dragOverRegion === region) {
            classNames.push("dragOver");
          }
          return (
            <div
              key={region}
              role="button"
              className={classNames.join(" ")}
              onClick={() => onRegionClick(region)}
              {...dropHandlers(region)}
            >
              {assigned.length > 0 ? (
                assigned.map(({ action, index }, stackPos) => (
                  <span
                    key={index}
                    className={
                      index === selectedIndex
                        ? "monitorChip selected"
                        : "monitorChip"
                    }
                    draggable
                    onClick={(event) => {
                      event.stopPropagation();
                      onChipSelect(index);
                    }}
                    onMouseDown={(event) => event.stopPropagation()}
                    onDragStart={(event) => {
                      event.stopPropagation();
                      event.dataTransfer.setData(
                        DRAG_ACTION_MIME,
                        String(index),
                      );
                      event.dataTransfer.effectAllowed = "move";
                    }}
                    onDragEnd={(event) => {
                      if (event.dataTransfer.dropEffect === "none") {
                        onRegionClear(index);
                      }
                    }}
                  >
                    {assigned.length > 1 && (
                      <span className="chipOrder">{stackPos + 1}</span>
                    )}
                    <span className="chipLabel">{actionLabel(action)}</span>
                    <span className="chipControls">
                      <button
                        type="button"
                        className="chipBtn"
                        aria-label="Bring forward"
                        disabled={stackPos === 0}
                        onClick={(event) => {
                          event.stopPropagation();
                          onMoveAction(index, -1);
                        }}
                      >
                        ↑
                      </button>
                      <button
                        type="button"
                        className="chipBtn"
                        aria-label="Send back"
                        disabled={stackPos === assigned.length - 1}
                        onClick={(event) => {
                          event.stopPropagation();
                          onMoveAction(index, 1);
                        }}
                      >
                        ↓
                      </button>
                      <button
                        type="button"
                        className="chipBtn"
                        aria-label="Remove placement"
                        onClick={(event) => {
                          event.stopPropagation();
                          onRegionClear(index);
                        }}
                      >
                        ✕
                      </button>
                    </span>
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
      <div
        role="button"
        className={[
          "fullTarget",
          placed.some(({ action }) => action.region === "centered")
            ? "filled"
            : "",
          dragOverRegion === "centered" ? "dragOver" : "",
        ]
          .filter(Boolean)
          .join(" ")}
        onClick={() => onRegionClick("centered")}
        {...dropHandlers("centered")}
      >
        <span className="fullTargetLabel">{t("region.centered")}</span>
        {placed
          .filter(({ action }) => action.region === "centered")
          .map(({ action, index }) => (
            <span
              key={index}
              className={
                index === selectedIndex ? "monitorChip selected" : "monitorChip"
              }
              draggable
              onClick={(event) => {
                event.stopPropagation();
                onChipSelect(index);
              }}
              onMouseDown={(event) => event.stopPropagation()}
              onDragStart={(event) => {
                event.stopPropagation();
                event.dataTransfer.setData(DRAG_ACTION_MIME, String(index));
                event.dataTransfer.effectAllowed = "move";
              }}
              onDragEnd={(event) => {
                if (event.dataTransfer.dropEffect === "none") {
                  onRegionClear(index);
                }
              }}
            >
              <span className="chipLabel">{actionLabel(action)}</span>
              <button
                type="button"
                className="chipBtn"
                aria-label="Remove placement"
                onClick={(event) => {
                  event.stopPropagation();
                  onRegionClear(index);
                }}
              >
                ✕
              </button>
            </span>
          ))}
      </div>
    </div>
  );
};

const ARRANGEMENT_MAX_WIDTH = 200;
const ARRANGEMENT_MAX_HEIGHT = 64;

/// Miniature of the real display arrangement (like macOS display settings);
/// click a display to aim the routine's layout at it. The main display
/// carries a menu-bar strip.
const DisplayArrangement = ({
  displays,
  selectedId,
  onPick,
}: {
  displays: DisplayInfo[];
  selectedId: number | null;
  onPick: (display: DisplayInfo) => void;
}) => {
  const minX = Math.min(...displays.map((d) => d.x));
  const minY = Math.min(...displays.map((d) => d.y));
  const maxX = Math.max(...displays.map((d) => d.x + d.width));
  const maxY = Math.max(...displays.map((d) => d.y + d.height));
  const scale = Math.min(
    ARRANGEMENT_MAX_WIDTH / (maxX - minX),
    ARRANGEMENT_MAX_HEIGHT / (maxY - minY),
  );
  // Fixed-size box with the arrangement letterboxed inside — the container
  // can never disagree with its absolutely-positioned children.
  const offsetX = (ARRANGEMENT_MAX_WIDTH - (maxX - minX) * scale) / 2;
  const offsetY = (ARRANGEMENT_MAX_HEIGHT - (maxY - minY) * scale) / 2;

  return (
    <div className="displayArrangement">
      {displays.map((display, order) => {
        const isSelected =
          selectedId === display.id || (selectedId === null && display.isMain);
        return (
          <button
            key={display.id}
            type="button"
            className={isSelected ? "displayRect selected" : "displayRect"}
            style={{
              left: offsetX + (display.x - minX) * scale,
              top: offsetY + (display.y - minY) * scale,
              width: display.width * scale,
              height: display.height * scale,
            }}
            onClick={() => onPick(display)}
          >
            {display.isMain && <span className="displayMenubar" />}
            <span className="displayNum">{order + 1}</span>
          </button>
        );
      })}
    </div>
  );
};

/// Shown only when regions are assigned but macOS has not granted the
/// Accessibility permission yet.
const PERMISSION_RECHECK_MS = 3000;

const PlacementPermissionHint = ({ needed }: { needed: boolean }) => {
  const t = useT();
  const [granted, setGranted] = useState<boolean | null>(null);

  // The user grants the permission in System Settings and comes back, so
  // keep re-checking until it flips instead of testing only once.
  useEffect(() => {
    if (!needed || granted === true) {
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
    const timer = window.setInterval(checkQuietly, PERMISSION_RECHECK_MS);
    window.addEventListener("focus", checkQuietly);
    return () => {
      cancelled = true;
      window.clearInterval(timer);
      window.removeEventListener("focus", checkQuietly);
    };
  }, [needed, granted]);

  if (!needed || granted !== false) {
    return null;
  }

  async function handleEnableClick() {
    setGranted(await repairAccessibilityPermission());
  }

  return (
    <div className="permissionHint">
      <span>
        {t("permission.hint")}{" "}
        <span className="permissionSub">{t("permission.restartHint")}</span>
      </span>
      <span className="permissionActions">
        <button
          type="button"
          className="ghostButton"
          onClick={handleEnableClick}
        >
          {t("permission.enable")}
        </button>
        <button
          type="button"
          className="ghostButton"
          onClick={() => void restartApp()}
        >
          {t("permission.restart")}
        </button>
      </span>
    </div>
  );
};

const ACTION_KINDS: ActionKind[] = ["open-app", "open-url"];

interface ActionCardProps {
  action: Action;
  index: number;
  stackPos: number;
  stackSize: number;
  isSelected: boolean;
  onSelect: () => void;
  onChange: (action: Action) => void;
  onClearRegion: () => void;
  onMove: (direction: -1 | 1) => void;
  onRemove: () => void;
  onReorderTo: (fromIndex: number) => void;
}

const ActionCard = ({
  action,
  index,
  stackPos,
  stackSize,
  isSelected,
  onSelect,
  onChange,
  onClearRegion,
  onMove,
  onRemove,
  onReorderTo,
}: ActionCardProps) => {
  const t = useT();
  const cardRef = useRef<HTMLDivElement>(null);
  const [isDragTarget, setIsDragTarget] = useState(false);

  // Keep the selected card visible — monitor-side interactions select
  // actions that may be scrolled out of view.
  useEffect(() => {
    if (isSelected) {
      cardRef.current?.scrollIntoView({ block: "nearest", behavior: "smooth" });
    }
  }, [isSelected, index]);

  const classNames = ["actionCard"];
  if (isSelected) {
    classNames.push("selected");
  }
  if (isDragTarget) {
    classNames.push("dragTarget");
  }

  return (
    <div
      ref={cardRef}
      className={classNames.join(" ")}
      onClick={onSelect}
      draggable
      onDragStart={(event) => {
        event.dataTransfer.setData(DRAG_ACTION_MIME, String(index));
        event.dataTransfer.effectAllowed = "move";
        onSelect();
      }}
      onDragOver={(event) => {
        event.preventDefault();
        event.dataTransfer.dropEffect = "move";
        setIsDragTarget(true);
      }}
      onDragLeave={() => setIsDragTarget(false)}
      onDrop={(event) => {
        event.preventDefault();
        setIsDragTarget(false);
        const from = Number(event.dataTransfer.getData(DRAG_ACTION_MIME));
        if (!Number.isNaN(from)) {
          onReorderTo(from);
        }
      }}
    >
      <span className="actionIndex">{stackPos + 1}</span>
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
            buildAction(
              action.type,
              event.target.value,
              action.region ?? null,
              action.display ?? null,
            ),
          )
        }
      />
      {action.type === "open-file" && (
        <button
          type="button"
          className="ghostButton browseButton"
          onClick={async (event) => {
            event.stopPropagation();
            const picked = await openFileDialog({ multiple: false });
            if (typeof picked === "string") {
              onChange(
                buildAction(
                  "open-file",
                  picked,
                  action.region ?? null,
                  action.display ?? null,
                ),
              );
            }
          }}
        >
          {t("editor.browse")}
        </button>
      )}
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
          disabled={stackPos === 0}
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
          disabled={stackPos === stackSize - 1}
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

