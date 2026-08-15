import { createContext, type ReactNode, useContext, useEffect, useState } from "react";
import { defaultSettings, loadSettings, type PackmasterSettings, saveSettings } from "./settings";

interface SettingsContextValue {
  settings: PackmasterSettings;
  /** Merges a partial update into the current settings and persists it. */
  updateSettings: (patch: Partial<PackmasterSettings>) => void;
  isLoaded: boolean;
}

const SettingsContext = createContext<SettingsContextValue | null>(null);

export function SettingsProvider({ children }: { children: ReactNode }) {
  const [settings, setSettings] = useState<PackmasterSettings>(defaultSettings);
  const [isLoaded, setIsLoaded] = useState(false);

  useEffect(() => {
    loadSettings().then((loaded) => {
      setSettings(loaded);
      setIsLoaded(true);
    });
  }, []);

  const updateSettings = (patch: Partial<PackmasterSettings>) => {
    setSettings((current) => {
      const next = { ...current, ...patch };
      // Fire-and-forget: every change is remembered and re-proposed next
      // launch (asked for directly) without making the UI wait on disk I/O.
      saveSettings(next);
      return next;
    });
  };

  return (
    <SettingsContext.Provider value={{ settings, updateSettings, isLoaded }}>
      {children}
    </SettingsContext.Provider>
  );
}

export function useSettings(): SettingsContextValue {
  const context = useContext(SettingsContext);
  if (!context) {
    throw new Error("useSettings must be used within a SettingsProvider");
  }
  return context;
}
