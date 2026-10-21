// Apply preset (SPECS §8): pick a recipe; each card says exactly what it does, and the real result
// for the first image is shown live.
import { useEffect, useState } from "react";
import { ChevronLeft } from "lucide-react";
import { Button } from "../components/Button";
import { EstimateCard } from "../components/EstimateCard";
import { presetSummary } from "../lib/format";
import { listPresets } from "../lib/ipc";
import type { FileInfo, JobRequest, Preset } from "../lib/types";

interface Props {
  files: FileInfo[];
  onBack: () => void;
  onRun: (request: JobRequest) => void;
}

export function PresetView({ files, onBack, onRun }: Props) {
  const [presets, setPresets] = useState<Preset[] | null>(null);
  const [chosen, setChosen] = useState<string>();
  const preset = presets?.find((p) => p.id === chosen);

  useEffect(() => {
    listPresets()
      .then((list) => {
        setPresets(list);
        setChosen(list[0]?.id);
      })
      .catch(() => setPresets([]));
  }, []);

  const request: JobRequest | null = preset ? { action: "apply_preset", preset } : null;

  return (
    <form
      onSubmit={(e) => {
        e.preventDefault();
        if (request) onRun(request);
      }}
      className="flex flex-col gap-3 px-4 pb-3"
    >
      <div className="flex items-center gap-1">
        <button
          type="button"
          onClick={onBack}
          aria-label="Back"
          className="-ml-1.5 flex size-7 items-center justify-center rounded-control text-secondary hover:bg-surface-hover"
        >
          <ChevronLeft size={18} strokeWidth={1.75} />
        </button>
        <h2 className="type-title text-primary">Apply preset</h2>
      </div>

      <div role="radiogroup" aria-label="Preset" className="flex max-h-[220px] flex-col gap-1 overflow-y-auto">
        {!presets && [0, 1, 2].map((i) => <div key={i} className="skeleton h-12 rounded-control" />)}
        {presets?.length === 0 && <p className="type-body text-secondary">No presets found.</p>}
        {presets?.map((p) => {
          const on = p.id === chosen;
          return (
            <button
              key={p.id}
              type="button"
              role="radio"
              aria-checked={on}
              onClick={() => setChosen(p.id)}
              className={`flex flex-col items-start rounded-control px-2.5 py-1.5 text-left ${
                on ? "bg-surface-selected inset-ring inset-ring-accent" : "bg-surface hover:bg-surface-hover"
              }`}
            >
              <span className={`type-body-strong ${on ? "text-accent" : "text-primary"}`}>{p.label}</span>
              <span className="type-caption text-secondary tabular-nums">{presetSummary(p)}</span>
            </button>
          );
        })}
      </div>
      {preset?.note && <p className="type-caption text-tertiary">{preset.note}</p>}

      <EstimateCard path={files[0].path} request={request} label={files.length > 1 ? "First image" : "Result"} />

      <div className="flex gap-2">
        <Button onClick={onBack}>Back</Button>
        <Button type="submit" variant="primary" className="flex-1" disabled={!preset} autoFocus>
          {files.length > 1 ? `Apply to ${files.length} images` : "Apply preset"}
        </Button>
      </div>
    </form>
  );
}
