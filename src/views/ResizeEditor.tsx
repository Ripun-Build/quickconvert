// Visual resize: drag the image's corner or edges, or type a size. Both stay in sync live.
import { useEffect, useRef, useState } from "react";
import { ChevronLeft, Link2, Unlink2 } from "lucide-react";
import { Button } from "../components/Button";
import { preview as loadPreview } from "../lib/ipc";
import {
  defaultSize,
  dragTo,
  percentOf,
  resizeRequest,
  scaled,
  setSide,
  type Handle,
  type Size,
} from "../lib/resize";
import type { FileInfo, JobRequest, Preview } from "../lib/types";

/** Room for the image inside the 720 px editor. */
const CANVAS = { width: 672, height: 300 };
const PRESETS = [25, 50, 75, 100];

interface Props {
  files: FileInfo[];
  /** Settings → Conversions → resize long edge. */
  defaultLongEdge?: number;
  onBack: () => void;
  onRun: (request: JobRequest) => void;
}

export function ResizeEditor({ files, defaultLongEdge, onBack, onRun }: Props) {
  const [image, setImage] = useState<Preview | null>(null);
  const [failed, setFailed] = useState(false);
  const [size, setSize] = useState<Size>({ width: 1, height: 1 });
  const [locked, setLocked] = useState(true);
  const [dragging, setDragging] = useState<Handle | null>(null);
  const boxRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    let live = true;
    loadPreview(files[0].path)
      .then((p) => {
        if (!live) return;
        setImage(p);
        setSize(defaultSize(p, defaultLongEdge));
      })
      .catch(() => live && setFailed(true));
    return () => {
      live = false;
    };
  }, [files]);

  const original = image ?? { width: 1, height: 1 };
  const fit = Math.min(CANVAS.width / original.width, CANVAS.height / original.height);
  const box = { width: original.width * fit, height: original.height * fit };
  const many = files.length > 1;

  const onPointerMove = (e: React.PointerEvent) => {
    if (!dragging || !boxRef.current) return;
    const rect = boxRef.current.getBoundingClientRect();
    const point = { x: (e.clientX - rect.left) / fit, y: (e.clientY - rect.top) / fit };
    setSize((s) => dragTo(dragging, point, s, original, locked));
  };

  const startDrag = (handle: Handle) => (e: React.PointerEvent) => {
    e.preventDefault();
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    setDragging(handle);
  };

  // Arrow keys on the corner handle: 1 % steps (Shift: 10 %).
  const onHandleKey = (e: React.KeyboardEvent) => {
    const dir = e.key === "ArrowRight" || e.key === "ArrowDown" ? 1 : e.key === "ArrowLeft" || e.key === "ArrowUp" ? -1 : 0;
    if (!dir) return;
    e.preventDefault();
    const step = (e.shiftKey ? 0.1 : 0.01) * dir;
    setSize((s) => scaled(original, s.width / original.width + step));
  };

  const submit = (e: React.FormEvent) => {
    e.preventDefault();
    if (image) onRun(resizeRequest(size, image, locked, files.length));
  };

  const note = !many
    ? `Original ${original.width} × ${original.height}`
    : locked
      ? `Previewing the first of ${files.length} images · each is scaled to ${percentOf(size, original)}%`
      : `All ${files.length} images become ${size.width} × ${size.height} px`;

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
        <h2 className="type-title text-primary">Resize</h2>
        <span className="type-caption ml-auto truncate text-tertiary">{image ? note : ""}</span>
      </div>

      {/* Canvas: dashed outline = original size; the live image shrinks inside it. */}
      <div
        className="flex items-center justify-center rounded-card bg-surface"
        style={{ height: CANVAS.height + 24 }}
        onPointerMove={onPointerMove}
        onPointerUp={() => setDragging(null)}
        onPointerCancel={() => setDragging(null)}
      >
        {failed && <p className="type-body text-secondary">This image couldn't be previewed.</p>}
        {!image && !failed && (
          <div className="skeleton rounded-control" style={{ width: CANVAS.height * 1.33, height: CANVAS.height }} />
        )}
        {image && (
          <div
            ref={boxRef}
            className="relative outline-1 outline-separator outline-dashed"
            style={{ width: box.width, height: box.height }}
          >
            <img src={image.url} alt="" draggable={false} className="absolute inset-0 size-full opacity-15" />
            <div
              className="absolute top-0 left-0"
              style={{ width: size.width * fit, height: size.height * fit }}
            >
              <img
                src={image.url}
                alt="Resized preview"
                draggable={false}
                className="size-full object-fill shadow-control"
              />
              <span className="type-caption pointer-events-none absolute right-2 bottom-2 rounded-kbd bg-glass px-1.5 py-0.5 whitespace-nowrap text-primary tabular-nums">
                {size.width} × {size.height}
              </span>
              <span
                onPointerDown={startDrag("right")}
                aria-hidden
                className="absolute top-1/2 -right-1.5 h-8 w-3 -translate-y-1/2 cursor-ew-resize rounded-pill bg-surface-raised shadow-control"
              />
              <span
                onPointerDown={startDrag("bottom")}
                aria-hidden
                className="absolute -bottom-1.5 left-1/2 h-3 w-8 -translate-x-1/2 cursor-ns-resize rounded-pill bg-surface-raised shadow-control"
              />
              <span
                role="slider"
                tabIndex={0}
                aria-label="Image size"
                aria-valuemin={1}
                aria-valuemax={100}
                aria-valuenow={Math.round(percentOf(size, original))}
                aria-valuetext={`${size.width} by ${size.height} pixels`}
                onPointerDown={startDrag("corner")}
                onKeyDown={onHandleKey}
                className="absolute -right-2 -bottom-2 size-4 cursor-nwse-resize rounded-pill border-2 border-accent bg-surface-raised shadow-control"
              />
            </div>
          </div>
        )}
      </div>

      <div className="flex flex-wrap items-center gap-2">
        <DimField label="Width" value={size.width} onCommit={(v) => setSize((s) => setSide("width", v, s, original, locked))} />
        <button
          type="button"
          onClick={() => setLocked((l) => !l)}
          aria-pressed={locked}
          aria-label={locked ? "Aspect ratio locked" : "Aspect ratio unlocked"}
          title={locked ? "Aspect ratio locked" : "Aspect ratio unlocked"}
          className={`flex size-[30px] items-center justify-center rounded-control ${locked ? "text-accent" : "text-tertiary"} hover:bg-surface-hover`}
        >
          {locked ? <Link2 size={16} strokeWidth={1.75} /> : <Unlink2 size={16} strokeWidth={1.75} />}
        </button>
        <DimField label="Height" value={size.height} onCommit={(v) => setSize((s) => setSide("height", v, s, original, locked))} />
        <span className="type-caption text-tertiary">px</span>

        <div className="ml-auto flex gap-1">
          {PRESETS.map((pct) => (
            <button
              key={pct}
              type="button"
              onClick={() => setSize(scaled(original, pct / 100))}
              className={`type-caption h-[26px] rounded-control px-2 tabular-nums ${
                image && percentOf(size, original) === pct ? "bg-surface-selected text-accent" : "bg-surface text-secondary"
              }`}
            >
              {pct}%
            </button>
          ))}
        </div>
      </div>

      <div className="flex gap-2">
        <Button onClick={onBack}>Back</Button>
        <Button type="submit" variant="primary" disabled={!image} className="flex-1" autoFocus>
          Resize
          {many && <span className="opacity-70">· {files.length} images</span>}
        </Button>
      </div>
    </form>
  );
}

/** Number field that keeps a draft while typing and updates the size on every valid keystroke. */
function DimField({ label, value, onCommit }: { label: string; value: number; onCommit: (v: number) => void }) {
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
