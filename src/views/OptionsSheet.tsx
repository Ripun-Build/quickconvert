// Options for one action (SPECS §5): back chevron + title, only that action's options, Enter runs.
import { useState } from "react";
import { ChevronLeft } from "lucide-react";
import { Button } from "../components/Button";
import { NumberField, Slider, Toggle } from "../components/Fields";
import { Segmented } from "../components/Segmented";
import type { ActionDef, ActionSpec, OptionField, OptionValues } from "../lib/actions";

interface Props {
  action: ActionDef;
  spec: ActionSpec;
  fileCount: number;
  /** User defaults that replace the spec's (e.g. Settings → JPEG quality). */
  overrides?: OptionValues;
  onBack: () => void;
  onRun: (values: OptionValues) => void;
}

export function OptionsSheet({ action, spec, fileCount, overrides, onBack, onRun }: Props) {
  const [values, setValues] = useState<OptionValues>({ ...spec.defaults, ...overrides });
  const set = (key: string, value: OptionValues[string]) => setValues((v) => ({ ...v, [key]: value }));
  const visible = spec.fields.filter((f) => !f.visible || f.visible(values));
  const invalid = visible.some((f) => f.kind === "text" && !f.valid(String(values[f.key] ?? "")));

  const submit = (e: React.FormEvent) => {
    e.preventDefault();
    if (invalid) return;
    // Empty number fields fall back to their defaults.
    const clean = Object.fromEntries(
      Object.entries(values).map(([k, v]) => [k, typeof v === "number" && Number.isNaN(v) ? spec.defaults[k] : v]),
    );
    onRun(clean);
  };

  return (
    <form onSubmit={submit} className="flex flex-col gap-3 px-4 pb-3">
      <div className="flex items-center gap-1">
        <button
          type="button"
          onClick={onBack}
          aria-label="Back"
          className="-ml-1.5 flex size-7 items-center justify-center rounded-control text-secondary hover:bg-surface-hover"
        >
          <ChevronLeft size={18} strokeWidth={1.75} />
        </button>
        <h2 className="type-title text-primary">{action.label}</h2>
      </div>

      {visible.map((field) => (
        <div key={field.key} className="flex flex-col gap-1.5">
          {field.kind !== "toggle" && <span className="type-caption text-secondary">{field.label}</span>}
          <FieldControl field={field} value={values[field.key]} onChange={(v) => set(field.key, v)} />
        </div>
      ))}

      <Button type="submit" variant="primary" autoFocus disabled={invalid} className="mt-1 w-full">
        {spec.submit}
        {fileCount > 1 && <span className="opacity-70">· {fileCount} files</span>}
      </Button>
    </form>
  );
}

function FieldControl({
  field,
  value,
  onChange,
}: {
  field: OptionField;
  value: OptionValues[string];
  onChange: (v: OptionValues[string]) => void;
}) {
  switch (field.kind) {
    case "segmented":
      return <Segmented label={field.label} value={String(value)} choices={field.choices} onChange={onChange} />;
    case "slider":
      return (
        <Slider
          label={field.label}
          value={Number(value)}
          min={field.min}
          max={field.max}
          step={field.step}
          unit={field.unit}
          onChange={onChange}
        />
      );
    case "number":
      return (
        <NumberField
          label={field.label}
          value={Number(value)}
          min={field.min}
          max={field.max}
          unit={field.unit}
          onChange={onChange}
        />
      );
    case "toggle":
      return <Toggle label={field.label} checked={Boolean(value)} onChange={onChange} />;
    case "text": {
      const text = String(value ?? "");
      const bad = text.trim() !== "" && !field.valid(text);
      return (
        <div className="flex flex-col gap-1">
          <input
            autoFocus
            value={text}
            onChange={(e) => onChange(e.target.value)}
            placeholder={field.placeholder}
            aria-label={field.label}
            aria-invalid={bad}
            className={`type-body h-[30px] rounded-control bg-surface px-2 text-primary tabular-nums outline-none placeholder:text-tertiary focus:ring-[3px] ${
              bad ? "ring-1 ring-danger focus:ring-danger" : "focus:ring-focus-ring"
            }`}
          />
          <span className={`type-caption ${bad ? "text-error" : "text-tertiary"}`}>{field.hint}</span>
        </div>
      );
    }
  }
}
