// Shown when no files are selected (or the selection can't be read): drop target + "Choose files…".
import { FileUp } from "lucide-react";

export type DropZoneHint = "permission" | "unsupported" | undefined;

const HINTS: Record<Exclude<DropZoneHint, undefined>, string> = {
  permission:
    "To use your Finder selection, allow access in System Settings → Privacy & Security → Automation.",
  unsupported: "These files aren't supported yet. Try images (JPG, PNG, WebP…) or PDFs.",
};

interface Props {
  hint: DropZoneHint;
  hotkey: string;
  dragging: boolean;
  onChoose: () => void;
}

export function DropZone({ hint, hotkey, dragging, onChoose }: Props) {
  return (
    <div className="p-3">
      <div
        className={`flex flex-col items-center gap-3 rounded-card border border-dashed px-4 py-6 text-center transition-colors duration-150 ${
          dragging ? "border-accent bg-surface-selected" : "border-separator"
        }`}
      >
        <FileUp size={24} strokeWidth={1.5} className={dragging ? "text-accent" : "text-tertiary"} />
        <div>
          <p className="type-body-strong text-primary">Drop files here</p>
          <p className="type-caption mt-1 text-secondary">
            {hint ? HINTS[hint] : `Or select files in Finder and press ${hotkey || "the shortcut"}.`}
          </p>
        </div>
        <button
          type="button"
          onClick={onChoose}
          className="type-body-strong h-[30px] rounded-control bg-surface px-3 text-primary inset-ring inset-ring-separator active:scale-[0.97]"
        >
          Choose files…
        </button>
      </div>
    </div>
  );
}
