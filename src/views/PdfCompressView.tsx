// Make PDF smaller: three plain-language levels and the real result for the first PDF, measured by
// actually compressing it in Rust (nothing is saved until the button is pressed).
import { useEffect, useRef, useState } from "react";
import { ArrowRight, ChevronLeft } from "lucide-react";
import { Button } from "../components/Button";
import { formatBytes } from "../lib/format";
import { isAppError, pdfCompressEstimate } from "../lib/ipc";
import type { FileInfo, JobRequest, PdfEstimate, PdfLevel } from "../lib/types";

const LEVELS: { level: PdfLevel; label: string; hint: string }[] = [
  { level: "smallest", label: "Smallest", hint: "For email and uploads. Pictures look softer." },
  { level: "balanced", label: "Balanced", hint: "Good for most uses." },
  { level: "print", label: "Print", hint: "Keeps pictures sharp enough to print." },
];
/** Same rule as Rust (`pdf_compress::MIN_SAVING`): under 5 % smaller isn't worth a copy. */
const MIN_SAVING = 0.05;

interface Props {
  files: FileInfo[];
  onBack: () => void;
  onRun: (request: JobRequest) => void;
}

export function PdfCompressView({ files, onBack, onRun }: Props) {
  const [level, setLevel] = useState<PdfLevel>("balanced");
  const [estimates, setEstimates] = useState<Partial<Record<PdfLevel, PdfEstimate>>>({});
  const [error, setError] = useState<string>();
  const latest = useRef(0);
  /** Levels already being measured, so switching back and forth doesn't measure twice. */
  const asked = useRef(new Set<PdfLevel>());
  const first = files[0];
  const many = files.length > 1;

  useEffect(() => {
    const ticket = ++latest.current;
    setError(undefined);
    if (asked.current.has(level)) return;
    asked.current.add(level);
    pdfCompressEstimate(first.path, level)
      .then((e) => setEstimates((all) => ({ ...all, [level]: e })))
      .catch((err) => {
        asked.current.delete(level); // try again next time this level is picked
        if (ticket === latest.current) setError(isAppError(err) ? err.message : "Couldn't work out the size.");
      });
  }, [first.path, level]);

  const estimate = estimates[level];
  const tooSmall = estimate && estimate.bytes > estimate.originalBytes * (1 - MIN_SAVING);

  return (
    <form
      onSubmit={(e) => {
        e.preventDefault();
        onRun({ action: "pdf_compress", level });
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
        <h2 className="type-title text-primary">Make PDF smaller</h2>
      </div>

      <div role="radiogroup" aria-label="How small" className="flex flex-col gap-1">
        {LEVELS.map((option) => {
          const on = option.level === level;
          return (
            <button
              key={option.level}
              type="button"
              role="radio"
              aria-checked={on}
              onClick={() => setLevel(option.level)}
              className={`flex flex-col items-start rounded-control px-2.5 py-1.5 text-left ${
                on ? "bg-surface-selected inset-ring inset-ring-accent" : "bg-surface hover:bg-surface-hover"
              }`}
            >
              <span className={`type-body-strong ${on ? "text-link" : "text-primary"}`}>{option.label}</span>
              <span className="type-caption text-secondary">{option.hint}</span>
            </button>
          );
        })}
      </div>

      <div className="flex flex-col gap-0.5 rounded-card bg-surface px-3 py-2.5" aria-live="polite">
        <span className="type-caption text-tertiary">{many ? `First PDF · ${first.name}` : "Result"}</span>
        {error ? (
          <span className="type-body text-error">{error}</span>
        ) : estimate ? (
          <>
            <span className="type-title flex items-center gap-1.5 tabular-nums">
              <span className="text-secondary">{formatBytes(estimate.originalBytes)}</span>
              <ArrowRight size={14} strokeWidth={2} className="text-tertiary" />
              <span className="text-primary">{formatBytes(estimate.bytes)}</span>
            </span>
            {tooSmall && (
              <span className="type-caption text-secondary">Already about as small as it gets: no copy will be saved.</span>
            )}
          </>
        ) : (
          <span className="flex items-center gap-2">
            <span className="skeleton h-5 w-40 rounded-kbd" />
            <span className="type-caption text-tertiary">Measuring…</span>
          </span>
        )}
      </div>

      <div className="flex gap-2">
        <Button onClick={onBack}>Back</Button>
        <Button type="submit" variant="primary" className="flex-1" autoFocus>
          Make smaller
          {many && <span className="opacity-70">· {files.length} PDFs</span>}
        </Button>
      </div>
    </form>
  );
}
