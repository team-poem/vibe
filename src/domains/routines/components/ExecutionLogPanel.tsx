import { useLanguage, useT } from "../../../shared/i18n/LanguageContext";
import { useExecutionLog } from "../useExecutionLog";
import type { ExecutionRecord } from "../types";

/// Recent routine runs with per-action results (PRD 7.6). Failures name
/// the exact action that failed.
export const ExecutionLogPanel = () => {
  const t = useT();
  const records = useExecutionLog();

  return (
    <section className="logPanel" aria-label="Recent runs">
      <h2 className="editorSectionTitle">{t("log.title")}</h2>
      {records.length === 0 ? (
        <p className="logEmpty">{t("log.empty")}</p>
      ) : (
        <ul className="logList">
          {records.map((record) => (
            <LogEntry key={record.atEpochMs} record={record} />
          ))}
        </ul>
      )}
    </section>
  );
};

const TIME_LOCALES = { en: "en-US", ko: "ko-KR" } as const;

const LogEntry = ({ record }: { record: ExecutionRecord }) => {
  const language = useLanguage();
  const time = new Date(record.atEpochMs).toLocaleTimeString(
    TIME_LOCALES[language],
  );
  const failures = record.outcomes.filter((outcome) => !outcome.success);

  return (
    <li className={record.success ? "logEntry" : "logEntry failed"}>
      <div className="logEntryHeader">
        <span className="logEntryStatus">{record.success ? "✓" : "✕"}</span>
        <span className="logEntryName">{record.routineName}</span>
        <span className="logEntryTime">{time}</span>
      </div>
      {failures.length > 0 && (
        <ul className="logFailures">
          {failures.map((outcome, index) => (
            <li key={index}>
              {outcome.label} — {outcome.detail}
            </li>
          ))}
        </ul>
      )}
    </li>
  );
};
