import { useState } from "react";

import { ExecutionLogPanel } from "./domains/routines/components/ExecutionLogPanel";
import { RoutineEditor } from "./domains/routines/components/RoutineEditor";
import { RoutineSidebar } from "./domains/routines/components/RoutineSidebar";
import { SettingsView } from "./domains/settings/components/SettingsView";
import { useRoutines } from "./domains/routines/useRoutines";
import type { Routine } from "./domains/routines/types";
import { LanguageProvider, useT } from "./shared/i18n/LanguageContext";
import type { Language } from "./shared/i18n/messages";
import { useAppliedTheme } from "./shared/theme";
import "./App.css";

type View = "routines" | "settings";

export default function App() {
  const {
    config,
    error,
    saveRoutine,
    deleteRoutine,
    setActiveRoutine,
    setLanguage,
    setTheme,
  } = useRoutines();
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [view, setView] = useState<View>("routines");
  useAppliedTheme(config?.theme ?? "system");

  if (!config) {
    return (
      <main className="app loading">
        <p>{error ?? "Loading…"}</p>
      </main>
    );
  }

  if (config.language === null) {
    return <LanguageOnboarding onPick={setLanguage} />;
  }

  const selectedRoutine =
    config.routines.find((r) => r.id === selectedId) ??
    config.routines.find((r) => r.id === config.activeRoutineId) ??
    config.routines[0] ??
    null;

  async function handleCreateRoutine() {
    const created = await saveRoutine(NEW_ROUTINE_TEMPLATE);
    if (created) {
      setSelectedId(created.id);
      setView("routines");
    }
  }

  async function handleSaveRoutine(routine: Routine) {
    await saveRoutine(routine);
  }

  async function handleDeleteRoutine(id: string) {
    await deleteRoutine(id);
    setSelectedId(null);
  }

  const handleSelectRoutine = (id: string) => {
    setSelectedId(id);
    setView("routines");
  };

  return (
    <LanguageProvider language={config.language}>
      <main className="app">
        <RoutineSidebar
          routines={config.routines}
          activeRoutineId={config.activeRoutineId}
          selectedId={selectedRoutine?.id ?? null}
          isSettingsOpen={view === "settings"}
          onSelect={handleSelectRoutine}
          onCreate={handleCreateRoutine}
          onDelete={handleDeleteRoutine}
          onOpenSettings={() => setView("settings")}
        />

        <div className="mainPane">
          {view === "settings" ? (
            <SettingsView
              language={config.language}
              theme={config.theme}
              onChangeLanguage={setLanguage}
              onChangeTheme={setTheme}
            />
          ) : selectedRoutine ? (
            <RoutineEditor
              key={selectedRoutine.id}
              routine={selectedRoutine}
              isActive={selectedRoutine.id === config.activeRoutineId}
              onSave={handleSaveRoutine}
              onDelete={handleDeleteRoutine}
              onActivate={setActiveRoutine}
            />
          ) : (
            <EmptyEditorPane onCreate={handleCreateRoutine} />
          )}
          {view === "routines" && <ExecutionLogPanel />}
        </div>

        {error && <p className="appError">{error}</p>}
      </main>
    </LanguageProvider>
  );
}

const NEW_ROUTINE_TEMPLATE: Routine = {
  id: "",
  name: "New Routine",
  actions: [],
};

/// First-launch screen, shown before any language is chosen — so it is
/// deliberately bilingual.
const LanguageOnboarding = ({
  onPick,
}: {
  onPick: (language: Language) => Promise<void>;
}) => {
  return (
    <main className="onboarding">
      <h1 className="onboardingBrand">V.I.B.E</h1>
      <p className="onboardingTagline">👏👏</p>
      <p className="onboardingQuestion">
        언어를 선택하세요 · Choose your language
      </p>
      <div className="onboardingChoices">
        <button
          type="button"
          className="onboardingChoice"
          onClick={() => onPick("ko")}
        >
          한국어
        </button>
        <button
          type="button"
          className="onboardingChoice"
          onClick={() => onPick("en")}
        >
          English
        </button>
      </div>
    </main>
  );
};

const EmptyEditorPane = ({ onCreate }: { onCreate: () => Promise<void> }) => {
  const t = useT();
  return (
    <section className="editor empty">
      <p>{t("editor.empty")}</p>
      <button type="button" className="primaryButton" onClick={onCreate}>
        {t("sidebar.new")}
      </button>
    </section>
  );
};
