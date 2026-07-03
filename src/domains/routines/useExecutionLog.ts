import { useEffect, useState } from "react";

import { listen } from "@tauri-apps/api/event";

import { fetchExecutionLog } from "./api";
import type { ExecutionRecord } from "./types";

/// Mirrors the backend's execution log ring buffer; refreshed whenever the
/// engine finishes running a routine.
export const useExecutionLog = (): ExecutionRecord[] => {
  const [records, setRecords] = useState<ExecutionRecord[]>([]);

  useEffect(() => {
    async function loadLog() {
      try {
        setRecords(await fetchExecutionLog());
      } catch {
        // The log is diagnostic; a failed fetch just leaves it stale.
      }
    }
    void loadLog();

    const unlisten = listen("exec-log://updated", () => {
      void loadLog();
    });
    return () => {
      void unlisten.then((dispose) => dispose());
    };
  }, []);

  return records;
};
