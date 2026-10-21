// Width/height field for the visual editors: "W 1600" in a small rounded box.
import { useEffect, useState } from "react";

/** Number field that keeps a draft while typing and updates the size on every valid keystroke. */
export function DimField({ label, value, onCommit }: { label: string; value: number; onCommit: (v: number) => void }) {
  const [draft, setDraft] = useState(String(value));
  const [editing, setEditing] = useState(false);
  useEffect(() => {
    if (!editing) setDraft(String(value));
  }, [value, editing]);

  return (
    <label className="flex h-[30px] w-[104px] items-center gap-1.5 rounded-control bg-surface px-2 focus-within:ring-[3px] focus-within:ring-focus-ring">
      <span className="type-caption text-tertiary">{label[0]}</span>
      <input
        inputMode="numeric"
        aria-label={label}
        value={draft}
        onFocus={() => setEditing(true)}
        onBlur={() => {
          setEditing(false);
          setDraft(String(value));
        }}
        onChange={(e) => {
          const digits = e.target.value.replace(/\D/g, "");
          setDraft(digits);
          if (digits) onCommit(Number(digits));
        }}
        className="type-body min-w-0 flex-1 bg-transparent text-primary tabular-nums outline-none"
      />
    </label>
  );
}
