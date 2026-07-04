import { useEffect } from "react";

/// Persisted theme preference; "system" follows the macOS appearance.
export type ThemeSetting = "system" | "light" | "dark";

/// Stamps the resolved theme onto <html data-theme="..."> so the CSS
/// palette variables switch, tracking the OS appearance in system mode.
export const useAppliedTheme = (setting: ThemeSetting) => {
  useEffect(() => {
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const apply = () => {
      const resolved =
        setting === "system" ? (media.matches ? "dark" : "light") : setting;
      document.documentElement.dataset.theme = resolved;
    };
    apply();
    if (setting !== "system") {
      return;
    }
    media.addEventListener("change", apply);
    return () => media.removeEventListener("change", apply);
  }, [setting]);
};
