import { useT } from "../../../shared/i18n/LanguageContext";
import { useArmedConfirm } from "../../../shared/useArmedConfirm";
import type { Routine } from "../types";

interface RoutineSidebarProps {
  routines: Routine[];
  activeRoutineId: string | null;
  selectedId: string | null;
  isSettingsOpen: boolean;
  onSelect: (id: string) => void;
  onCreate: () => void;
  onDelete: (id: string) => void;
  onOpenSettings: () => void;
}

export const RoutineSidebar = ({
  routines,
  activeRoutineId,
  selectedId,
  isSettingsOpen,
  onSelect,
  onCreate,
  onDelete,
  onOpenSettings,
}: RoutineSidebarProps) => {
  const t = useT();

  return (
    <aside className="sidebar">
      <header className="brand">
        <h1 className="brandName">V.I.B.E</h1>
      </header>

      <nav className="routineList" aria-label="Routines">
        {routines.map((routine) => (
          <RoutineListItem
            key={routine.id}
            routine={routine}
            isActive={routine.id === activeRoutineId}
            isSelected={!isSettingsOpen && routine.id === selectedId}
            onSelect={onSelect}
            onDelete={onDelete}
          />
        ))}
        {routines.length === 0 && (
          <p className="routineListEmpty">{t("sidebar.empty")}</p>
        )}
      </nav>

      <button type="button" className="ghostButton newRoutine" onClick={onCreate}>
        {t("sidebar.new")}
      </button>
      <button
        type="button"
        className={isSettingsOpen ? "settingsLink on" : "settingsLink"}
        onClick={onOpenSettings}
      >
        ⚙ {t("sidebar.settings")}
      </button>
    </aside>
  );
};

interface RoutineListItemProps {
  routine: Routine;
  isActive: boolean;
  isSelected: boolean;
  onSelect: (id: string) => void;
  onDelete: (id: string) => void;
}

const RoutineListItem = ({
  routine,
  isActive,
  isSelected,
  onSelect,
  onDelete,
}: RoutineListItemProps) => {
  const t = useT();
  const { armed, trigger } = useArmedConfirm(() => onDelete(routine.id));
  const className = isSelected ? "routineItem selected" : "routineItem";

  return (
    <div className={className} onClick={() => onSelect(routine.id)}>
      <span className="routineItemName">{routine.name}</span>
      {isActive && (
        <span className="activeDot" title={t("sidebar.activeRoutine")} />
      )}
      <button
        type="button"
        className={armed ? "routineDelete armed" : "routineDelete"}
        aria-label={t("editor.delete")}
        onClick={(event) => {
          event.stopPropagation();
          trigger();
        }}
      >
        {armed ? t("delete.confirm") : "✕"}
      </button>
    </div>
  );
};
