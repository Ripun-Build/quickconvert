// Segmented control (DESIGN §4): pill track, raised thumb that slides between segments.
import { useId } from "react";
import { motion } from "framer-motion";

interface Props {
  label: string;
  value: string;
  choices: { value: string; label: string }[];
  onChange: (value: string) => void;
}

export function Segmented({ label, value, choices, onChange }: Props) {
  const id = useId();
  const index = choices.findIndex((c) => c.value === value);

  const onKeyDown = (e: React.KeyboardEvent) => {
    const step = e.key === "ArrowRight" ? 1 : e.key === "ArrowLeft" ? -1 : 0;
    if (!step) return;
    e.preventDefault();
    const next = choices[(index + step + choices.length) % choices.length];
    onChange(next.value);
  };

  return (
    <div role="radiogroup" aria-label={label} onKeyDown={onKeyDown} className="flex rounded-control bg-surface p-0.5">
      {choices.map((choice) => {
        const selected = choice.value === value;
        return (
          <button
            key={choice.value}
            type="button"
            role="radio"
            aria-checked={selected}
            tabIndex={selected ? 0 : -1}
            onClick={() => onChange(choice.value)}
            className={`type-body relative h-[26px] flex-1 rounded-kbd px-2 ${selected ? "text-primary" : "text-secondary"}`}
          >
            {selected && (
              <motion.span
                layoutId={`segment-${id}`}
                className="absolute inset-0 rounded-kbd bg-surface-raised shadow-control"
                transition={{ type: "spring", stiffness: 520, damping: 40 }}
              />
            )}
            <span className="relative">{choice.label}</span>
          </button>
        );
      })}
    </div>
  );
}
