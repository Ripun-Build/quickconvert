// Presets: list with add / duplicate / delete, an editor for the selected one, and Restore.
// Edits save shortly after typing stops; Rust stores only what differs from the built-ins.
import { useEffect, useRef, useState } from "react";
import { Copy, Plus, RotateCcw, Trash2 } from "lucide-react";
import { Segmented } from "../components/Segmented";
import { presetSummary } from "../lib/format";
import { isAppError, presetsForEditor, restorePresets, savePresets } from "../lib/ipc";
import type { EditorPreset, Preset } from "../lib/types";

const SAVE_DELAY_MS = 400;
type SizeMode = "exact" | "aspect" | "long" | "keep";

const sizeModeOf = (p: Preset): SizeMode =>
  p.width && p.height ? "exact" : p.aspect ? "aspect" : p.longEdge ? "long" : "keep";

const newId = () => `custom-${Date.now().toString(36)}`;

export function PresetsTab() {
  const [entries, setEntries] = useState<EditorPreset[] | null>(null);
  const [selected, setSelected] = useState<string>();
  const [error, setError] = useState<string>();
  const timer = useRef<ReturnType<typeof setTimeout>>(undefined);

  useEffect(() => {
    presetsForEditor()
      .then((list) => {
        setEntries(list);
        setSelected(list[0]?.preset.id);
      })
      .catch(() => setError("Presets couldn't be loaded."));
  }, []);

  const persist = (list: Preset[], now = false) => {
    clearTimeout(timer.current);
    const run = () =>
      savePresets(list)
        .then((saved) => {
          setEntries(saved);
          setError(undefined);
        })
        .catch((err) => setError(isAppError(err) ? err.message : "Couldn't save presets."));
    if (now) void run();
    else timer.current = setTimeout(run, SAVE_DELAY_MS);
  };

  if (!entries) return <div className="skeleton h-40 rounded-card" />;
  const list = entries.map((e) => e.preset);
  const current = entries.find((e) => e.preset.id === selected);

  const edit = (next: Preset) => {
    setEntries(entries.map((e) => (e.preset.id === next.id ? { ...e, preset: next } : e)));
    persist(list.map((p) => (p.id === next.id ? next : p)));
  };
  const add = () => {
    const preset: Preset = { id: newId(), label: "New preset", format: "jpg", maxKB: 200 };
    setSelected(preset.id);
    persist([...list, preset], true);
  };
  const duplicate = () => {
    if (!current) return;
    const preset = { ...current.preset, id: newId(), label: `${current.preset.label} copy` };
    setSelected(preset.id);
    persist([...list, preset], true);
  };
  const remove = () => {
    if (!current) return;
    const index = list.findIndex((p) => p.id === current.preset.id);
    const rest = list.filter((p) => p.id !== current.preset.id);
    setSelected(rest[Math.min(index, rest.length - 1)]?.id);
    persist(rest, true);
  };
  const restore = () =>
    restorePresets()
      .then((restored) => {
        setEntries(restored);
        if (!restored.some((e) => e.preset.id === selected)) setSelected(restored[0]?.preset.id);
      })
      .catch(() => setError("Couldn't restore the built-in presets."));

  return (
    <>
      <div className="flex flex-col overflow-hidden rounded-card bg-surface inset-ring inset-ring-separator">
        <div role="listbox" aria-label="Presets" className="max-h-[168px] overflow-y-auto p-1">
          {entries.map(({ preset, builtin }) => {
            const on = preset.id === selected;
            return (
              <button
                key={preset.id}
                type="button"
                role="option"
                aria-selected={on}
                onClick={() => setSelected(preset.id)}
                className={`flex w-full items-center gap-2 rounded-row px-2.5 py-1.5 text-left ${
                  on ? "bg-surface-selected" : "hover:bg-surface-hover"
                }`}
              >
                <span className="min-w-0 flex-1">
                  <span className={`type-body-strong block truncate ${on ? "text-link" : "text-primary"}`}>{preset.label}</span>
                  <span className="type-caption block truncate text-secondary">{presetSummary(preset)}</span>
                </span>
                {builtin && <span className="type-caption shrink-0 rounded-kbd bg-surface px-1.5 text-tertiary">Example</span>}
              </button>
            );
          })}
          {entries.length === 0 && <p className="type-body px-2.5 py-2 text-secondary">No presets. Add one below.</p>}
        </div>
        <div className="flex items-center gap-1 border-t border-separator px-1.5 py-1">
          <ToolButton label="Add" onClick={add} icon={<Plus size={14} />} />
          <ToolButton label="Duplicate" onClick={duplicate} icon={<Copy size={14} />} disabled={!current} />
          <ToolButton label="Delete" onClick={remove} icon={<Trash2 size={14} />} disabled={!current} />
          <button type="button" onClick={() => void restore()} className="type-caption ml-auto flex items-center gap-1 px-2 text-link hover:underline">
            <RotateCcw size={12} />
            Restore built-in presets
          </button>
        </div>
      </div>

      {error && <p className="type-caption text-error">{error}</p>}
      {current && <PresetForm key={current.preset.id} preset={current.preset} builtin={current.builtin} onChange={edit} />}
    </>
  );
}

function ToolButton({ label, icon, onClick, disabled }: { label: string; icon: React.ReactNode; onClick: () => void; disabled?: boolean }) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled}
      className="type-caption flex h-6 items-center gap-1 rounded-control px-2 text-primary hover:bg-surface-hover disabled:opacity-40"
    >
      {icon}
      {label}
    </button>
  );
}

function PresetForm({ preset, builtin, onChange }: { preset: Preset; builtin: boolean; onChange: (p: Preset) => void }) {
  const mode = sizeModeOf(preset);
  const set = (patch: Partial<Preset>) => onChange({ ...preset, ...patch });
  const setMode = (next: SizeMode) => {
    const cleared: Preset = { ...preset, width: undefined, height: undefined, aspect: undefined, longEdge: undefined };
    if (next === "exact") onChange({ ...cleared, width: 600, height: 600 });
    else if (next === "aspect") onChange({ ...cleared, aspect: "35:45" });
    else if (next === "long") onChange({ ...cleared, longEdge: 1600 });
    else onChange(cleared);
  };

  return (
    <div className="flex flex-col gap-3 rounded-card bg-surface p-3 inset-ring inset-ring-separator">
      <p className="type-caption text-secondary">
        {presetSummary(preset)}
        {builtin && " · built-in example; your changes are kept until you restore"}
      </p>

      <Field label="Name">
        <TextInput value={preset.label} onChange={(label) => set({ label })} />
      </Field>

      <Field label="Format">
        <Segmented
          label="Format"
          value={preset.format}
          choices={[
            { value: "jpg", label: "JPG" },
            { value: "png", label: "PNG" },
            { value: "webp", label: "WebP" },
          ]}
          onChange={(format) => set({ format: format as Preset["format"] })}
        />
      </Field>

      <Field label="Size">
        <Segmented
          label="Size"
          value={mode}
          choices={[
            { value: "exact", label: "Exact" },
            { value: "aspect", label: "Ratio" },
            { value: "long", label: "Long edge" },
            { value: "keep", label: "Keep" },
          ]}
          onChange={(v) => setMode(v as SizeMode)}
        />
        <div className="flex items-center gap-2">
          {mode === "exact" && (
            <>
              <NumberInput label="Width" unit="px" value={preset.width} onChange={(width) => set({ width })} />
              <span className="type-caption text-tertiary">×</span>
              <NumberInput label="Height" unit="px" value={preset.height} onChange={(height) => set({ height })} />
              <span className="type-caption text-tertiary">cropped from the centre to fit</span>
            </>
          )}
          {mode === "aspect" && (
            <>
              <TextInput value={preset.aspect ?? ""} placeholder="35:45" onChange={(aspect) => set({ aspect })} width="w-24" />
              <span className="type-caption text-tertiary">width : height, cropped from the centre</span>
            </>
          )}
          {mode === "long" && (
            <>
              <NumberInput label="Long edge" unit="px" value={preset.longEdge} onChange={(longEdge) => set({ longEdge })} />
              <span className="type-caption text-tertiary">never enlarged</span>
            </>
          )}
          {mode === "keep" && <span className="type-caption text-tertiary">Keeps each image's size.</span>}
        </div>
      </Field>

      <div className="grid grid-cols-3 gap-3">
        <Field label="At least">
          <NumberInput label="Minimum size" unit="KB" value={preset.minKB} onChange={(minKB) => set({ minKB })} />
        </Field>
        <Field label="At most">
          <NumberInput label="Maximum size" unit="KB" value={preset.maxKB} onChange={(maxKB) => set({ maxKB })} />
        </Field>
        <Field label="DPI">
          <NumberInput label="DPI" unit="dpi" value={preset.dpi} onChange={(dpi) => set({ dpi })} />
        </Field>
      </div>

      <Field label="Note">
        <TextInput
          value={preset.note ?? ""}
          placeholder="e.g. Check the portal's official notice."
          onChange={(note) => set({ note: note || undefined })}
        />
      </Field>
    </div>
  );
}

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="flex flex-col gap-1.5">
      <span className="type-caption text-secondary">{label}</span>
      {children}
    </div>
  );
}

function TextInput({
  value,
  onChange,
  placeholder,
  width = "w-full",
}: {
  value: string;
  onChange: (v: string) => void;
  placeholder?: string;
  width?: string;
}) {
  return (
    <input
      value={value}
      placeholder={placeholder}
      onChange={(e) => onChange(e.target.value)}
      className={`type-body h-[30px] ${width} rounded-control bg-surface-raised px-2 text-primary shadow-control outline-none placeholder:text-tertiary focus:ring-[3px] focus:ring-focus-ring`}
    />
  );
}

/** Optional positive whole number; blank means "not set". */
function NumberInput({
  label,
  unit,
  value,
  onChange,
}: {
  label: string;
  unit: string;
  value?: number;
  onChange: (v: number | undefined) => void;
}) {
  return (
    <label className="flex h-[30px] w-full min-w-[84px] items-center gap-1 rounded-control bg-surface-raised px-2 shadow-control focus-within:ring-[3px] focus-within:ring-focus-ring">
      <input
        inputMode="numeric"
        aria-label={label}
        value={value ?? ""}
        placeholder="–"
        onChange={(e) => {
          const digits = e.target.value.replace(/\D/g, "").slice(0, 6);
          onChange(digits ? Number(digits) : undefined);
        }}
        className="type-body min-w-0 flex-1 bg-transparent text-primary tabular-nums outline-none placeholder:text-tertiary"
      />
      <span className="type-caption text-tertiary">{unit}</span>
    </label>
  );
}
