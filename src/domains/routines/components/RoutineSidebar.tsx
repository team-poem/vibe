import { useLanguage, useT } from "../../../shared/i18n/LanguageContext";
import type { Language } from "../../../shared/i18n/messages";
import type { Routine } from "../types";

interface RoutineSidebarProps {
  routines: Routine[];
  activeRoutineId: string | null;
  selectedId: string | null;
  onSelect: (id: string) => void;
  onCreate: () => void;
  onChangeLanguage: (language: Language) => void;
}

export const RoutineSidebar = ({
  routines,
  activeRoutineId,
  selectedId,
  onSelect,
  onCreate,
  onChangeLanguage,
}: RoutineSidebarProps) => {
  const t = useT();

  return (
    <aside className="sidebar">
      <header className="brand">
        <h1 className="brandName">V.I.B.E</h1>
        <p className="brandTagline">{t("sidebar.tagline")}</p>
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
          <p className="routineListEmpty">{t("sidebar.empty")}</p>
        )}
      </nav>

      <button type="button" className="ghostButton newRoutine" onClick={onCreate}>
        {t("sidebar.new")}
      </button>
      <LanguageSwitch onChange={onChangeLanguage} />
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
  const t = useT();
  const className = isSelected ? "routineItem selected" : "routineItem";

  return (
    <button
      type="button"
      className={className}
      onClick={() => onSelect(routine.id)}
    >
      <span className="routineItemName">{routine.name}</span>
      {isActive && (
        <span className="activeDot" title={t("sidebar.activeRoutine")} />
      )}
    </button>
  );
};

const LANGUAGE_OPTIONS: { value: Language; label: string }[] = [
  { value: "en", label: "EN" },
  { value: "ko", label: "한국어" },
];

const LanguageSwitch = ({
  onChange,
}: {
  onChange: (language: Language) => void;
}) => {
  const current = useLanguage();

  return (
    <div className="langSwitch" role="group" aria-label="Language">
      {LANGUAGE_OPTIONS.map((option) => (
        <button
          key={option.value}
          type="button"
          className={
            option.value === current ? "langButton on" : "langButton"
          }
          onClick={() => onChange(option.value)}
        >
          {option.label}
        </button>
      ))}
    </div>
  );
};
