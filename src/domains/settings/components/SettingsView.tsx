import { useEffect, useState } from "react";

import {
  checkAccessibilityPermission,
  fetchAutostart,
  repairAccessibilityPermission,
  setAutostartInSystem,
  testMicrophone,
} from "../../routines/api";
import { useT } from "../../../shared/i18n/LanguageContext";
import type { Language, MessageKey } from "../../../shared/i18n/messages";
import type { ThemeSetting } from "../../../shared/theme";
import type { ClapSensitivity } from "../../routines/types";

const APP_VERSION = "0.1.4";

interface SettingsViewProps {
  language: Language;
  theme: ThemeSetting;
  sensitivity: ClapSensitivity;
  onChangeLanguage: (language: Language) => void;
  onChangeTheme: (theme: ThemeSetting) => void;
  onChangeSensitivity: (sensitivity: ClapSensitivity) => void;
}

export const SettingsView = ({
  language,
  theme,
  sensitivity,
  onChangeLanguage,
  onChangeTheme,
  onChangeSensitivity,
}: SettingsViewProps) => {
  const t = useT();

  return (
    <section className="editor settings">
      <h1 className="settingsTitle">{t("settings.title")}</h1>

      <h2 className="editorSectionTitle">{t("settings.themeSection")}</h2>
      <ThemePicker current={theme} onChange={onChangeTheme} />

      <h2 className="editorSectionTitle">{t("settings.languageSection")}</h2>
      <LanguagePicker current={language} onChange={onChangeLanguage} />

      <h2 className="editorSectionTitle">
        {t("settings.sensitivitySection")}
      </h2>
      <SensitivityPicker current={sensitivity} onChange={onChangeSensitivity} />
      <p className="settingsHint">{t("settings.sensitivityHint")}</p>

      <h2 className="editorSectionTitle">{t("settings.generalSection")}</h2>
      <AutostartToggle />

      <h2 className="editorSectionTitle">{t("settings.permissionsSection")}</h2>
      <div className="settingsGroup">
        <MicrophoneRow />
        <AccessibilityRow />
      </div>

      <h2 className="editorSectionTitle">{t("settings.aboutSection")}</h2>
      <div className="settingsGroup">
        <div className="settingsRow">
          <div className="settingsRowText">
            <span className="settingsRowName">V.I.B.E</span>
            <span className="settingsRowDesc">
              {t("settings.version")} {APP_VERSION}
            </span>
          </div>
        </div>
      </div>
    </section>
  );
};

const THEME_OPTIONS: { value: ThemeSetting; labelKey: MessageKey }[] = [
  { value: "system", labelKey: "theme.system" },
  { value: "light", labelKey: "theme.light" },
  { value: "dark", labelKey: "theme.dark" },
];

const ThemePicker = ({
  current,
  onChange,
}: {
  current: ThemeSetting;
  onChange: (theme: ThemeSetting) => void;
}) => {
  const t = useT();
  return (
    <div className="segmented" role="group" aria-label="Theme">
      {THEME_OPTIONS.map((option) => (
        <button
          key={option.value}
          type="button"
          className={
            option.value === current ? "segmentedItem on" : "segmentedItem"
          }
          onClick={() => onChange(option.value)}
        >
          {t(option.labelKey)}
        </button>
      ))}
    </div>
  );
};

const SENSITIVITY_OPTIONS: { value: ClapSensitivity; labelKey: MessageKey }[] =
  [
    { value: "low", labelKey: "sensitivity.low" },
    { value: "medium", labelKey: "sensitivity.medium" },
    { value: "high", labelKey: "sensitivity.high" },
  ];

const SensitivityPicker = ({
  current,
  onChange,
}: {
  current: ClapSensitivity;
  onChange: (sensitivity: ClapSensitivity) => void;
}) => {
  const t = useT();
  return (
    <div className="segmented" role="group" aria-label="Clap sensitivity">
      {SENSITIVITY_OPTIONS.map((option) => (
        <button
          key={option.value}
          type="button"
          className={
            option.value === current ? "segmentedItem on" : "segmentedItem"
          }
          onClick={() => onChange(option.value)}
        >
          {t(option.labelKey)}
        </button>
      ))}
    </div>
  );
};

const LANGUAGE_OPTIONS: { value: Language; label: string }[] = [
  { value: "ko", label: "한국어" },
  { value: "en", label: "English" },
];

const LanguagePicker = ({
  current,
  onChange,
}: {
  current: Language;
  onChange: (language: Language) => void;
}) => {
  return (
    <div className="segmented" role="group" aria-label="Language">
      {LANGUAGE_OPTIONS.map((option) => (
        <button
          key={option.value}
          type="button"
          className={
            option.value === current ? "segmentedItem on" : "segmentedItem"
          }
          onClick={() => onChange(option.value)}
        >
          {option.label}
        </button>
      ))}
    </div>
  );
};

const AutostartToggle = () => {
  const t = useT();
  const [enabled, setEnabled] = useState(false);

  useEffect(() => {
    let cancelled = false;
    async function loadAutostart() {
      const value = await fetchAutostart();
      if (!cancelled) {
        setEnabled(value);
      }
    }
    void loadAutostart();
    return () => {
      cancelled = true;
    };
  }, []);

  async function handleToggle() {
    setEnabled(await setAutostartInSystem(!enabled));
  }

  return (
    <div className="settingsGroup">
      <div className="settingsRow">
        <div className="settingsRowText">
          <span className="settingsRowName">{t("settings.autostart")}</span>
          <span className="settingsRowDesc">{t("settings.autostartHint")}</span>
        </div>
        <button
          type="button"
          role="switch"
          aria-checked={enabled}
          className={enabled ? "switch on" : "switch"}
          onClick={handleToggle}
        >
          <span className="switchKnob" />
        </button>
      </div>
    </div>
  );
};

const MicrophoneRow = () => {
  const t = useT();
  const [result, setResult] = useState<string | null>(null);
  const [failed, setFailed] = useState(false);

  async function handleTestClick() {
    try {
      setResult(await testMicrophone());
      setFailed(false);
    } catch (cause) {
      setResult(String(cause));
      setFailed(true);
    }
  }

  return (
    <div className="settingsRow">
      <div className="settingsRowText">
        <span className="settingsRowName">{t("settings.micName")}</span>
        <span className="settingsRowDesc">
          {result === null
            ? t("settings.micDesc")
            : failed
              ? result
              : `${t("settings.micGranted")}${result}`}
        </span>
      </div>
      <button type="button" className="ghostButton" onClick={handleTestClick}>
        {t("settings.micTest")}
      </button>
    </div>
  );
};

const PERMISSION_RECHECK_MS = 3000;

const AccessibilityRow = () => {
  const t = useT();
  const [granted, setGranted] = useState<boolean | null>(null);

  useEffect(() => {
    if (granted === true) {
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
  }, [granted]);

  async function handleEnableClick() {
    setGranted(await repairAccessibilityPermission());
  }

  return (
    <div className="settingsRow">
      <div className="settingsRowText">
        <span className="settingsRowName">{t("settings.accName")}</span>
        <span className="settingsRowDesc">{t("settings.accDesc")}</span>
      </div>
      {granted ? (
        <span className="settingsGranted">✓ {t("settings.accGranted")}</span>
      ) : (
        <button
          type="button"
          className="ghostButton"
          onClick={handleEnableClick}
        >
          {t("settings.accEnable")}
        </button>
      )}
    </div>
  );
};
