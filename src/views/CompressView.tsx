// Compress (SPECS §6–7): to a target size or by quality, with the real resulting size shown live.
import { useState } from "react";
import { ChevronLeft } from "lucide-react";
import { Button } from "../components/Button";
import { EstimateCard } from "../components/EstimateCard";
import { Slider } from "../components/Fields";
import { Segmented } from "../components/Segmented";
import type { CompressTarget, FileInfo, JobRequest } from "../lib/types";

const TARGETS_KB = [20, 50, 100, 200, 500, 1000];
const DEFAULT_TARGET_KB = 200;
const DEFAULT_QUALITY = 75;

const label = (kb: number) => (kb >= 1000 ? `${kb / 1000} MB` : `${kb} KB`);
const isPng = (f: FileInfo) => !/\.(jpe?g|webp)$/i.test(f.name);

interface Props {
  files: FileInfo[];
  onBack: () => void;
  onRun: (request: JobRequest) => void;
}

export function CompressView({ files, onBack, onRun }: Props) {
  const [mode, setMode] = useState<"size" | "quality">("size");
  const [targetKb, setTargetKb] = useState(DEFAULT_TARGET_KB);
  const [customDraft, setCustomDraft] = useState("");
  const [quality, setQuality] = useState(DEFAULT_QUALITY);
  const first = files[0];
  const lossless = isPng(first);

  const target: CompressTarget = mode === "size" ? { kind: "size", maxKb: targetKb } : { kind: "quality", quality };
  const request: JobRequest = { action: "image_compress", target };

  return (
    <form
      onSubmit={(e) => {
        e.preventDefault();
        onRun(request);
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
        <h2 className="type-title text-primary">Compress</h2>
      </div>

      <Segmented
        label="Compress by"
        value={mode}
        choices={[
          { value: "size", label: "Target size" },
          { value: "quality", label: "Quality" },
        ]}
        onChange={(v) => setMode(v as "size" | "quality")}
      />

      {mode === "size" ? (
        <section className="flex flex-col gap-1.5">
          <span className="type-caption text-secondary">Make each file at most</span>
          <div className="grid grid-cols-3 gap-1">
            {TARGETS_KB.map((kb) => (
              <button
                key={kb}
                type="button"
                onClick={() => {
                  setTargetKb(kb);
                  setCustomDraft("");
                }}
                className={`type-body h-[30px] rounded-control tabular-nums ${
                  targetKb === kb && !customDraft
                    ? "bg-surface-selected text-link inset-ring inset-ring-accent"
                    : "bg-surface text-primary hover:bg-surface-hover"
                }`}
              >
                {label(kb)}
              </button>
            ))}
          </div>
          <label className="flex h-[30px] items-center gap-2 rounded-control bg-surface px-2 focus-within:ring-[3px] focus-within:ring-focus-ring">
            <span className="type-caption text-tertiary">Other</span>
            <input
              inputMode="numeric"
              aria-label="Other size in KB"
              placeholder="e.g. 75"
              value={customDraft}
              onChange={(e) => {
                const digits = e.target.value.replace(/\D/g, "").slice(0, 6);
                setCustomDraft(digits);
                if (Number(digits) > 0) setTargetKb(Number(digits));
              }}
              className="type-body min-w-0 flex-1 bg-transparent text-primary tabular-nums outline-none placeholder:text-tertiary"
            />
            <span className="type-caption text-tertiary">KB</span>
          </label>
          {lossless && (
            <span className="type-caption text-tertiary">
              PNG keeps every pixel, so it's made smaller by reducing its size in pixels.
            </span>
          )}
        </section>
      ) : (
        <section className="flex flex-col gap-1.5">
          <span className="type-caption text-secondary">Quality</span>
          {lossless ? (
            <span className="type-caption text-tertiary">
              PNG has no quality setting. It's saved with the strongest compression that loses nothing.
            </span>
          ) : (
            <>
              <Slider label="Quality" value={quality} min={1} max={100} step={1} onChange={setQuality} />
              <span className="type-caption text-tertiary">Lower quality gives smaller files. 70–85 looks good for most photos.</span>
            </>
          )}
        </section>
      )}

      <EstimateCard path={first.path} request={request} label={files.length > 1 ? "First image" : "Result"} />

      <div className="flex gap-2">
        <Button onClick={onBack}>Back</Button>
        <Button type="submit" variant="primary" className="flex-1" autoFocus>
          {files.length > 1 ? `Compress ${files.length} images` : "Compress"}
        </Button>
      </div>
    </form>
  );
}
