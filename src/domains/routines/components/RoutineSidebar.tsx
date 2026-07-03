import type { Routine } from "../types";

interface RoutineSidebarProps {
  routines: Routine[];
  activeRoutineId: string | null;
  selectedId: string | null;
  onSelect: (id: string) => void;
  onCreate: () => void;
}

export const RoutineSidebar = ({
  routines,
  activeRoutineId,
  selectedId,
  onSelect,
  onCreate,
}: RoutineSidebarProps) => {
  return (
    <aside className="sidebar">
      <header className="brand">
        <h1 className="brandName">V.I.B.E</h1>
        <p className="brandTagline">Clap twice. Your setup appears.</p>
      </header>

      <nav className="routineList" aria-label="Routines">
        {routines.map((routine) => (
          <RoutineListItem
            key={routine.id}
            routine={routine}
            isActive={routine.id === activeRoutineId}
            isSelected={routine.id === selectedId}
            onSelect={onSelect}
          />
        ))}
        {routines.length === 0 && (
          <p className="routineListEmpty">No routines yet.</p>
        )}
      </nav>

      <button type="button" className="ghostButton newRoutine" onClick={onCreate}>
        + New routine
      </button>
    </aside>
  );
};

interface RoutineListItemProps {
  routine: Routine;
  isActive: boolean;
  isSelected: boolean;
  onSelect: (id: string) => void;
}

const RoutineListItem = ({
  routine,
  isActive,
  isSelected,
  onSelect,
}: RoutineListItemProps) => {
  const className = isSelected ? "routineItem selected" : "routineItem";

  return (
    <button
      type="button"
      className={className}
      onClick={() => onSelect(routine.id)}
    >
      <span className="routineItemName">{routine.name}</span>
      {isActive && <span className="activeDot" title="Active routine" />}
    </button>
  );
};
