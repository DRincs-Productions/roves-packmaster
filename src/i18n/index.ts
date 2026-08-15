import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import de from "./locales/de.json";
import en from "./locales/en.json";
import es from "./locales/es.json";
import fr from "./locales/fr.json";
import it from "./locales/it.json";
import ja from "./locales/ja.json";
import ko from "./locales/ko.json";
import ru from "./locales/ru.json";
import zh from "./locales/zh.json";

// Every UI string lives in one of these locale files — see this project's own
// CLAUDE.md: no hardcoded user-facing string is allowed outside of them.
export const resources = {
  en: { translation: en },
  it: { translation: it },
  zh: { translation: zh },
  ja: { translation: ja },
  ko: { translation: ko },
  es: { translation: es },
  fr: { translation: fr },
  ru: { translation: ru },
  de: { translation: de },
} as const;

export type SupportedLanguage = keyof typeof resources;

export const supportedLanguages: SupportedLanguage[] = [
  "en",
  "it",
  "zh",
  "ja",
  "ko",
  "es",
  "fr",
  "ru",
  "de",
];

export const languageNames: Record<SupportedLanguage, string> = {
  en: "English",
  it: "Italiano",
  zh: "中文",
  ja: "日本語",
  ko: "한국어",
  es: "Español",
  fr: "Français",
  ru: "Русский",
  de: "Deutsch",
};

// Persisted separately from the rest of Packmaster's settings (see
// src/lib/settings.ts) — the language choice needs to be available
// synchronously, before the settings store (which is itself async) has
// necessarily loaded, so the very first paint is already in the right
// language instead of flashing English first.
const STORAGE_KEY = "roves-packmaster:language";

function detectInitialLanguage(): SupportedLanguage {
  const stored = localStorage.getItem(STORAGE_KEY);
  if (stored && stored in resources) {
    return stored as SupportedLanguage;
  }
  const browserLanguage = navigator.language.slice(0, 2).toLowerCase();
  if (browserLanguage in resources) {
    return browserLanguage as SupportedLanguage;
  }
  return "en";
}

i18n.use(initReactI18next).init({
  resources,
  lng: detectInitialLanguage(),
  fallbackLng: "en",
  interpolation: {
    escapeValue: false,
  },
});

i18n.on("languageChanged", (lng) => {
  localStorage.setItem(STORAGE_KEY, lng);
});

export default i18n;
