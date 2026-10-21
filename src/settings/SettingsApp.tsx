// Settings window (SPECS §10): toolbar tabs General · Conversions · Presets. Changes apply at once.
import { useCallback, useEffect, useState } from "react";
import { LayoutList, Settings2, SlidersHorizontal, type LucideIcon } from "lucide-react";
import { applyAppearance, getSettings, isAppError, onSettingsChanged, saveSettings } from "../lib/ipc";
import type { Settings, SettingsView } from "../lib/types";
import { ConversionsTab } from "./ConversionsTab";
import { GeneralTab } from "./GeneralTab";
import { PresetsTab } from "./PresetsTab";

type Tab = "general" | "conversions" | "presets";
const TABS: { id: Tab; label: string; icon: LucideIcon }[] = [
  { id: "general", label: "General", icon: Settings2 },
  { id: "conversions", label: "Conversions", icon: SlidersHorizontal },
  { id: "presets", label: "Presets", icon: LayoutList },
];

export default function SettingsApp({ initialTab, notice }: { initialTab: string | null; notice: string | null }) {
  const [tab, setTab] = useState<Tab>(TABS.some((t) => t.id === initialTab) ? (initialTab as Tab) : "general");
  const [view, setView] = useState<SettingsView | null>(null);
  const [error, setError] = useState<string>();

  const show = useCallback((v: SettingsView) => {
    setView(v);
    applyAppearance(v.settings.appearance);
  }, []);

  useEffect(() => {
    getSettings().then(show).catch(() => setError("Settings couldn't be loaded."));
    const unlisten = onSettingsChanged(show);
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [show]);

  const update = useCallback(
    (patch: Partial<Settings>) => {
      if (!view) return;
      saveSettings({ ...view.settings, ...patch })
        .then((v) => {
          show(v);
          setError(undefined);
        })
        .catch((err) => setError(isAppError(err) ? err.message : "Couldn't save that setting."));
    },
    [view, show],
  );

  return (
    <div className="flex h-screen flex-col bg-solid text-primary">
      <nav className="flex shrink-0 justify-center gap-1 border-b border-separator px-4 pt-2 pb-1.5" aria-label="Settings sections">
        {TABS.map(({ id, label, icon: Icon }) => {
          const on = tab === id;
          return (
            <button
              key={id}
              type="button"
              onClick={() => setTab(id)}
              aria-current={on ? "page" : undefined}
              className={`flex w-24 flex-col items-center gap-0.5 rounded-control py-1.5 ${
                on ? "bg-surface-selected text-accent" : "text-secondary hover:bg-surface-hover"
              }`}
            >
              <Icon size={20} strokeWidth={1.6} />
              <span className="type-caption">{label}</span>
            </button>
          );
        })}
      </nav>

      <main className="flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto p-5">
        {error && <p className="type-caption rounded-control bg-surface px-3 py-2 text-danger">{error}</p>}
        {!view ? (
          <div className="skeleton h-40 rounded-card" />
        ) : tab === "general" ? (
          <GeneralTab view={view} highlightConflict={notice === "shortcut-conflict"} onChange={update} onView={show} />
        ) : tab === "conversions" ? (
          <ConversionsTab settings={view.settings} onChange={update} />
        ) : (
          <PresetsTab />
        )}
      </main>
    </div>
  );
}
