// Progress while a job runs (SPECS §5): bar, "3 of 12 · photo_03.jpg", Cancel (Esc also cancels).
import { Button } from "../components/Button";
import { truncateMiddle } from "../lib/format";

interface Props {
  title: string;
  /** Files finished or in progress (1-based for display). */
  index: number;
  count: number;
  fileName: string;
  percent: number;
  cancelling: boolean;
  onCancel: () => void;
}

export function Running({ title, index, count, fileName, percent, cancelling, onCancel }: Props) {
  return (
    <div className="flex flex-col gap-3 px-4 pb-3" aria-live="polite">
      <h2 className="type-title text-primary">{cancelling ? "Cancelling…" : `${title}…`}</h2>
      <div
        role="progressbar"
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={Math.round(percent)}
        className="h-1 overflow-hidden rounded-pill bg-separator"
      >
        <div
          className="h-full rounded-pill bg-accent transition-[width] duration-200 ease-out"
          style={{ width: `${Math.max(3, percent)}%` }}
        />
      </div>
      <p className="type-caption truncate text-secondary tabular-nums">
        {Math.min(index + 1, count)} of {count}
        {fileName && ` · ${truncateMiddle(fileName, 36)}`}
      </p>
      <Button onClick={onCancel} disabled={cancelling} className="self-end">
        Cancel
      </Button>
    </div>
  );
}
