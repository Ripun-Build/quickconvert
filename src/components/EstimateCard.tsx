// Live result of Compress / a preset on the first image: "4.1 MB → 48 KB · 1600×1200 · quality 78".
// Measured by really encoding the image in Rust, so the number is exact.
import { useEffect, useRef, useState } from "react";
import { ArrowRight } from "lucide-react";
import { formatBytes } from "../lib/format";
import { estimateOutput, isAppError } from "../lib/ipc";
import type { Estimate, JobRequest } from "../lib/types";

const DEBOUNCE_MS = 250;

export function EstimateCard({ path, request, label }: { path: string; request: JobRequest | null; label: string }) {
  const [estimate, setEstimate] = useState<Estimate | null>(null);
  const [error, setError] = useState<string>();
  const [loading, setLoading] = useState(false);
  const latest = useRef(0);
  const key = JSON.stringify(request);

  useEffect(() => {
    if (!request) return;
    const ticket = ++latest.current;
    setLoading(true);
    const timer = setTimeout(() => {
      estimateOutput(path, request)
        .then((e) => {
          if (ticket !== latest.current) return; // a newer setting is being measured
          setEstimate(e);
          setError(undefined);
        })
        .catch((err) => {
          if (ticket !== latest.current) return;
          setEstimate(null);
          setError(isAppError(err) ? err.message : "Couldn't work out the size.");
        })
        .finally(() => ticket === latest.current && setLoading(false));
    }, DEBOUNCE_MS);
    return () => clearTimeout(timer);
  }, [path, key]); // `key` is `request` serialised, so equal settings don't re-measure

  return (
    <div className="flex flex-col gap-0.5 rounded-card bg-surface px-3 py-2.5" aria-live="polite">
      <span className="type-caption text-tertiary">{label}</span>
      {error ? (
        <span className="type-body text-danger">{error}</span>
      ) : estimate ? (
        <>
          <span className={`type-title flex items-center gap-1.5 tabular-nums ${loading ? "opacity-50" : ""}`}>
            <span className="text-secondary">{formatBytes(estimate.originalBytes)}</span>
            <ArrowRight size={14} strokeWidth={2} className="text-tertiary" />
            <span className="text-primary">{formatBytes(estimate.bytes)}</span>
          </span>
          <span className="type-caption text-secondary tabular-nums">
            {estimate.width}×{estimate.height}
            {estimate.quality ? ` · quality ${estimate.quality}` : ""}
            {estimate.bytes >= estimate.originalBytes && " · already this small, the copy won't be smaller"}
          </span>
        </>
      ) : (
        <span className="skeleton h-5 w-40 rounded-kbd" />
      )}
    </div>
  );
}
