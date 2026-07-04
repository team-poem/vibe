import { createContext, useContext } from "react";
import type { ReactNode } from "react";

import { MESSAGES } from "./messages";
import type { Language, MessageKey } from "./messages";

const LanguageContext = createContext<Language>("en");

export const LanguageProvider = ({
  language,
  children,
}: {
  language: Language;
  children: ReactNode;
}) => {
  return (
    <LanguageContext.Provider value={language}>
      {children}
    </LanguageContext.Provider>
  );
};

export const useLanguage = (): Language => {
  return useContext(LanguageContext);
};

/// Translation lookup bound to the current language.
export const useT = (): ((key: MessageKey) => string) => {
  const language = useContext(LanguageContext);
  return (key) => MESSAGES[language][key];
};
