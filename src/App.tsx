import { useState } from "react";

import { RoutineEditor } from "./domains/routines/components/RoutineEditor";
import { RoutineSidebar } from "./domains/routines/components/RoutineSidebar";
import { useRoutines } from "./domains/routines/useRoutines";
import type { Routine } from "./domains/routines/types";
import "./App.css";

export default function App() {
  const { config, error, saveRoutine, deleteRoutine, setActiveRoutine } =
    useRoutines();
  const [selectedId, setSelectedId] = useState<string | null>(null);

  if (!config) {
    return (
      <main className="app loading">
        <p>{error ?? "Loading routines…"}</p>
      </main>
    );
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
    }
  }

  async function handleSaveRoutine(routine: Routine) {
    await saveRoutine(routine);
  }

  async function handleDeleteRoutine(id: string) {
    await deleteRoutine(id);
    setSelectedId(null);
  }

  return (
    <main className="app">
      <RoutineSidebar
        routines={config.routines}
        activeRoutineId={config.activeRoutineId}
        selectedId={selectedRoutine?.id ?? null}
        onSelect={setSelectedId}
        onCreate={handleCreateRoutine}
      />

      {selectedRoutine ? (
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

      {error && <p className="appError">{error}</p>}
    </main>
  );
}

const NEW_ROUTINE_TEMPLATE: Routine = {
  id: "",
  name: "New Routine",
  actions: [],
};

const EmptyEditorPane = ({ onCreate }: { onCreate: () => Promise<void> }) => {
  return (
    <section className="editor empty">
      <p>Create a routine to run on a double clap.</p>
      <button type="button" className="primaryButton" onClick={onCreate}>
        + New routine
      </button>
    </section>
  );
};
