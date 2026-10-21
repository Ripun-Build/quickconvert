// Slider, number field and toggle (DESIGN §4).

interface SliderProps {
  label: string;
  value: number;
  min: number;
  max: number;
  step: number;
  unit?: string;
  onChange: (value: number) => void;
}

export function Slider({ label, value, min, max, step, unit = "", onChange }: SliderProps) {
  const fill = ((value - min) / (max - min)) * 100;
  return (
    <div className="flex items-center gap-3">
      <input
        type="range"
        aria-label={label}
        min={min}
        max={max}
        step={step}
        value={value}
        onChange={(e) => onChange(Number(e.target.value))}
        style={{ "--fill": `${fill}%` } as React.CSSProperties}
        className="range-slider min-w-0 flex-1"
      />
      <span className="type-caption w-10 text-right text-secondary tabular-nums">
        {value}
        {unit}
      </span>
    </div>
  );
}

interface NumberFieldProps {
  label: string;
  value: number;
  min: number;
  max: number;
  unit: string;
  onChange: (value: number) => void;
}

export function NumberField({ label, value, min, max, unit, onChange }: NumberFieldProps) {
  return (
    <label className="flex h-[30px] items-center gap-2 rounded-control bg-surface px-2 focus-within:ring-[3px] focus-within:ring-focus-ring">
      <input
        inputMode="numeric"
        aria-label={label}
        value={Number.isNaN(value) ? "" : value}
        onChange={(e) => {
          const digits = e.target.value.replace(/\D/g, "");
          onChange(digits === "" ? NaN : Number(digits));
        }}
        onBlur={() => onChange(Math.min(max, Math.max(min, Number.isNaN(value) ? min : value)))}
        className="type-body min-w-0 flex-1 bg-transparent text-primary tabular-nums outline-none"
      />
      <span className="type-caption text-tertiary">{unit}</span>
    </label>
  );
}

export function Toggle({ label, checked, onChange }: { label: string; checked: boolean; onChange: (v: boolean) => void }) {
  return (
    <label className="type-body flex items-center gap-2 text-primary">
      <input
        type="checkbox"
        checked={checked}
        onChange={(e) => onChange(e.target.checked)}
        className="size-4 accent-accent"
      />
      {label}
    </label>
  );
}
