// General: shortcut, appearance, launch at login, auto-hide.
import { useRef, useState } from "react";
import { TriangleAlert } from "lucide-react";
import { Segmented } from "../components/Segmented";
import { isAppError, setLaunchAtLogin, setShortcut, suspendShortcut } from "../lib/ipc";
import { liveLabel, toAccelerator } from "../lib/shortcutKeys";
import type { Settings, SettingsView } from "../lib/types";
import { Group, Row, Switch } from "./ui";

interface Props {
  view: SettingsView;
  /** Opened because the shortcut was taken at startup. */
  highlightConflict: boolean;
  onChange: (patch: Partial<Settings>) => void;
  onView: (view: SettingsView) => void;
}

export function GeneralTab({ view, highlightConflict, onChange, onView }: Props) {
  const { settings } = view;
  const [shortcutError, setShortcutError] = useState<string>();
  const [loginError, setLoginError] = useState<string>();
  const conflict = view.shortcutConflict;

  const applyShortcut = (accelerator: string | null) =>
    setShortcut(accelerator)
      .then((v) => {
        onView(v);
        setShortcutError(undefined);
      })
      .catch((err) => setShortcutError(isAppError(err) ? err.message : "That shortcut can't be used."));

  return (
    <>
      {conflict && (
        <div
          role="alert"
          className={`flex items-start gap-2 rounded-card px-3 py-2.5 ${
            highlightConflict ? "bg-surface-selected inset-ring inset-ring-warning" : "bg-surface"
          }`}
        >
          <TriangleAlert size={16} strokeWidth={1.75} className="mt-0.5 shrink-0 text-caution" />
          <p className="type-body text-primary">{conflict}</p>
        </div>
      )}

      <Group title="Shortcut">
        <Row label="Open QuickConvert" description="Press it with files selected in Finder.">
          <ShortcutRecorder label={view.shortcutLabel} onRecorded={applyShortcut} />
          {settings.shortcut !== null && (
            <button
              type="button"
              onClick={() => void applyShortcut(null)}
              className="type-caption text-link hover:underline"
              title={`Back to ${view.defaultShortcutLabel}`}
            >
              Reset
            </button>
          )}
        </Row>
        {shortcutError && <p className="type-caption px-3 py-2 text-error">{shortcutError}</p>}
      </Group>

      <Group title="General">
        <Row label="Appearance">
          <div className="w-[216px]">
            <Segmented
              label="Appearance"
              value={settings.appearance}
              choices={[
                { value: "system", label: "System" },
                { value: "light", label: "Light" },
                { value: "dark", label: "Dark" },
              ]}
              onChange={(v) => onChange({ appearance: v as Settings["appearance"] })}
            />
          </div>
        </Row>
        <Row label="Launch at login" description="Start QuickConvert in the menu bar when you log in.">
          <Switch
            label="Launch at login"
            checked={view.launchAtLogin}
            onChange={(on) =>
              setLaunchAtLogin(on)
                .then((v) => {
                  onView(v);
                  setLoginError(undefined);
                })
                .catch(() => setLoginError("Couldn't change this. Check System Settings → General → Login Items."))
            }
          />
        </Row>
        {loginError && <p className="type-caption px-3 py-2 text-error">{loginError}</p>}
        <Row label="Hide after saving" description="Close the popup 2.5 seconds after a conversion finishes.">
          <Switch label="Hide after saving" checked={settings.autoHide} onChange={(on) => onChange({ autoHide: on })} />
        </Row>
      </Group>
    </>
  );
}

/** Click, then press the new shortcut. Esc cancels. The current shortcut is paused meanwhile. */
function ShortcutRecorder({ label, onRecorded }: { label: string; onRecorded: (accelerator: string) => Promise<void> }) {
  const [recording, setRecording] = useState(false);
  const [live, setLive] = useState("");
  const ref = useRef<HTMLButtonElement>(null);

  const start = () => {
    setRecording(true);
    setLive("");
    void suspendShortcut(true).catch(() => {});
  };
  const stop = () => {
    setRecording(false);
    setLive("");
    void suspendShortcut(false).catch(() => {});
  };

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (!recording) return;
    e.preventDefault();
    e.stopPropagation();
    const noModifiers = !e.metaKey && !e.ctrlKey && !e.altKey && !e.shiftKey;
    if (e.code === "Escape" && noModifiers) {
      stop();
      return;
    }
    setLive(liveLabel(e));
    const accelerator = toAccelerator(e);
    if (accelerator) {
      // Rust decides; on failure the old shortcut stays and the message shows below.
      void onRecorded(accelerator).finally(() => {
        stop();
        ref.current?.blur();
      });
    }
  };

  return (
    <button
      ref={ref}
      type="button"
      onClick={() => (recording ? stop() : start())}
      onKeyDown={onKeyDown}
      onKeyUp={(e) => recording && !e.metaKey && !e.ctrlKey && !e.altKey && !e.shiftKey && setLive("")}
      onBlur={() => recording && stop()}
      aria-label={recording ? "Press the new shortcut" : `Shortcut ${label}. Click to change.`}
      className={`type-body-strong h-[30px] min-w-[132px] rounded-control px-3 tabular-nums ${
        recording ? "bg-surface-selected text-link inset-ring inset-ring-accent" : "bg-surface-raised text-primary shadow-control"
      }`}
    >
      {recording ? live || "Type shortcut…" : label}
    </button>
  );
}
