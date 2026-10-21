// Visual crop: drag the box or its handles over the photo, pick a shape, turn the photo. The part
// being cropped away is dimmed.
import { useEffect, useRef, useState } from "react";
import { motion } from "framer-motion";
import { ChevronLeft, RotateCcw, RotateCw } from "lucide-react";
import { Button } from "../components/Button";
import { DimField } from "../components/DimField";
import {
  SHAPES,
  cropRequest,
  dragHandle,
  fitShape,
  moveBox,
  passportHint,
  roundBox,
  setBoxSide,
  turnBy,
  turnedSize,
  type Box,
  type CropHandle,
} from "../lib/crop";
import { preview as loadPreview } from "../lib/ipc";
import type { FileInfo, JobRequest, Preview, Turn } from "../lib/types";

/** Room for the photo inside the 720 px editor (shorter than Resize: there are two control rows). */
const CANVAS = { width: 672, height: 270 };
const TURN = { type: "spring", stiffness: 300, damping: 30 } as const;

const HANDLES: { handle: CropHandle; className: string }[] = [
  { handle: "nw", className: "-top-2 -left-2 size-4 cursor-nwse-resize" },
  { handle: "ne", className: "-top-2 -right-2 size-4 cursor-nesw-resize" },
  { handle: "sw", className: "-bottom-2 -left-2 size-4 cursor-nesw-resize" },
  { handle: "n", className: "-top-1 left-1/2 h-2 w-7 -translate-x-1/2 cursor-ns-resize" },
  { handle: "s", className: "-bottom-1 left-1/2 h-2 w-7 -translate-x-1/2 cursor-ns-resize" },
  { handle: "w", className: "top-1/2 -left-1 h-7 w-2 -translate-y-1/2 cursor-ew-resize" },
  { handle: "e", className: "top-1/2 -right-1 h-7 w-2 -translate-y-1/2 cursor-ew-resize" },
];

interface Drag {
  handle: CropHandle | "move";
  start: Box;
  /** Pointer position (image pixels) when the drag began. */
  from: { x: number; y: number };
}

interface Props {
  file: FileInfo;
  onBack: () => void;
  onRun: (request: JobRequest) => void;
}

export function CropEditor({ file, onBack, onRun }: Props) {
  const [image, setImage] = useState<Preview | null>(null);
  const [failed, setFailed] = useState(false);
  const [turn, setTurn] = useState<Turn>(0);
  /** On-screen rotation; keeps counting past 360 so every turn animates the short way. */
  const [angle, setAngle] = useState(0);
  const [shapeId, setShapeId] = useState("free");
  const [box, setBox] = useState<Box>({ x: 0, y: 0, width: 1, height: 1 });
  const [drag, setDrag] = useState<Drag | null>(null);
  const areaRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    let live = true;
    loadPreview(file.path)
      .then((p) => {
        if (!live) return;
        setImage(p);
        setBox(fitShape(p, null));
      })
      .catch(() => live && setFailed(true));
    return () => {
      live = false;
    };
  }, [file]);

  const original = image ?? { width: 1, height: 1 };
  const sized = turnedSize(original, turn);
  const ratio = SHAPES.find((s) => s.id === shapeId)?.ratio ?? null;
  const fit = Math.min(CANVAS.width / sized.width, CANVAS.height / sized.height);
  const shown = roundBox(box, sized);
  const place = { left: box.x * fit, top: box.y * fit, width: box.width * fit, height: box.height * fit };

  const pointAt = (e: React.PointerEvent, area: HTMLElement) => {
    const rect = area.getBoundingClientRect();
    return { x: (e.clientX - rect.left) / fit, y: (e.clientY - rect.top) / fit };
  };

  const startDrag = (handle: CropHandle | "move") => (e: React.PointerEvent) => {
    if (!areaRef.current) return;
    e.preventDefault();
    e.stopPropagation();
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    setDrag({ handle, start: box, from: pointAt(e, areaRef.current) });
  };

  const onPointerMove = (e: React.PointerEvent) => {
    if (!drag || !areaRef.current) return;
    const p = pointAt(e, areaRef.current);
    setBox(
      drag.handle === "move"
        ? moveBox(drag.start, p.x - drag.from.x, p.y - drag.from.y, sized)
        : dragHandle(drag.handle, drag.start, p, sized, ratio),
    );
  };

  /** Arrow key → step in image pixels (one screen pixel; Shift: ten). */
  const arrow = (e: React.KeyboardEvent) => {
    const step = (e.shiftKey ? 10 : 1) / fit;
    const dx = e.key === "ArrowRight" ? step : e.key === "ArrowLeft" ? -step : 0;
    const dy = e.key === "ArrowDown" ? step : e.key === "ArrowUp" ? -step : 0;
    if (!dx && !dy) return null;
    e.preventDefault();
    e.stopPropagation();
    return { dx, dy };
  };

  const onBoxKey = (e: React.KeyboardEvent) => {
    const d = arrow(e);
    if (d) setBox((b) => moveBox(b, d.dx, d.dy, sized));
  };

  // The bottom-right handle resizes from the keyboard; with a shape both sides grow together.
  const onCornerKey = (e: React.KeyboardEvent) => {
    const d = arrow(e);
    if (!d) return;
    setBox((b) => {
      const right = b.x + b.width;
      const bottom = b.y + b.height;
      if (!ratio) return dragHandle("se", b, { x: right + d.dx, y: bottom + d.dy }, sized, null);
      const step = d.dx || d.dy;
      return dragHandle("se", b, { x: right + step, y: bottom + step / ratio }, sized, ratio);
    });
  };

  const rotate = (step: 1 | -1) => {
    const next = turnBy(turn, step);
    setTurn(next);
    setAngle((a) => a + step * 90);
    setBox(fitShape(turnedSize(original, next), ratio));
  };

  const pickShape = (id: string) => {
    setShapeId(id);
    setBox(fitShape(sized, SHAPES.find((s) => s.id === id)?.ratio ?? null));
  };

  const reset = () => {
    setAngle((a) => a - (turn === 270 ? -90 : turn));
    setTurn(0);
    setShapeId("free");
    setBox(fitShape(original, null));
  };

  const submit = (e: React.FormEvent) => {
    e.preventDefault();
    if (image) onRun(cropRequest(box, sized, turn));
  };

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
        <h2 className="type-title text-primary">Crop</h2>
        <span className="type-caption ml-auto truncate text-tertiary">
          {image ? `Original ${original.width} × ${original.height}` : ""}
        </span>
      </div>

      <div
        className="flex items-center justify-center rounded-card bg-surface"
        style={{ height: CANVAS.height + 24 }}
        onPointerMove={onPointerMove}
        onPointerUp={() => setDrag(null)}
        onPointerCancel={() => setDrag(null)}
      >
        {failed && <p className="type-body text-secondary">This image couldn't be previewed.</p>}
        {!image && !failed && (
          <div className="skeleton rounded-control" style={{ width: CANVAS.height * 1.33, height: CANVAS.height }} />
        )}
        {image && (
          <motion.div
            ref={areaRef}
            className="relative shrink-0"
            initial={false}
            animate={{ width: sized.width * fit, height: sized.height * fit }}
            transition={TURN}
          >
            {/* Photo and dimming, clipped to the photo. The hole in the dimming is the crop box. */}
            <div className="pointer-events-none absolute inset-0 flex items-center justify-center overflow-hidden">
              <motion.img
                src={image.url}
                alt=""
                draggable={false}
                className="max-w-none shrink-0"
                initial={false}
                animate={{ rotate: angle, width: original.width * fit, height: original.height * fit }}
                transition={TURN}
              />
              <div className="absolute shadow-scrim" style={place} />
            </div>

            <div
              role="group"
              tabIndex={0}
              aria-label={`Crop area, ${shown.width} by ${shown.height} pixels. Arrow keys move it.`}
              onPointerDown={startDrag("move")}
              onKeyDown={onBoxKey}
              className="absolute cursor-move border border-crop-line"
              style={place}
            >
              {drag && (
                <div aria-hidden className="pointer-events-none absolute inset-0 opacity-60">
                  <span className="absolute inset-y-0 left-1/3 w-px bg-crop-line" />
                  <span className="absolute inset-y-0 left-2/3 w-px bg-crop-line" />
                  <span className="absolute inset-x-0 top-1/3 h-px bg-crop-line" />
                  <span className="absolute inset-x-0 top-2/3 h-px bg-crop-line" />
                </div>
              )}
              <span className="type-caption pointer-events-none absolute right-2 bottom-2 rounded-kbd bg-glass px-1.5 py-0.5 whitespace-nowrap text-primary tabular-nums">
                {shown.width} × {shown.height}
              </span>
              {HANDLES.map(({ handle, className }) => (
                <span
                  key={handle}
                  aria-hidden
                  onPointerDown={startDrag(handle)}
                  className={`absolute rounded-pill bg-crop-line shadow-control ${className}`}
                />
              ))}
              <span
                role="slider"
                tabIndex={0}
                aria-label="Crop size"
                aria-valuemin={1}
                aria-valuemax={sized.width}
                aria-valuenow={shown.width}
                aria-valuetext={`${shown.width} by ${shown.height} pixels`}
                onPointerDown={startDrag("se")}
                onKeyDown={onCornerKey}
                className="absolute -right-2 -bottom-2 size-4 cursor-nwse-resize rounded-pill border-2 border-accent bg-crop-line shadow-control"
              />
            </div>
          </motion.div>
        )}
      </div>

      <div className="flex items-center gap-2">
        <DimField label="Width" value={shown.width} onCommit={(v) => setBox((b) => setBoxSide("width", v, b, sized, ratio))} />
        <DimField label="Height" value={shown.height} onCommit={(v) => setBox((b) => setBoxSide("height", v, b, sized, ratio))} />
        <span className="type-caption text-tertiary">px</span>
        <div className="ml-2 flex gap-1">
          <IconButton label="Rotate left" onClick={() => rotate(-1)} disabled={!image}>
            <RotateCcw size={16} strokeWidth={1.75} />
          </IconButton>
          <IconButton label="Rotate right" onClick={() => rotate(1)} disabled={!image}>
            <RotateCw size={16} strokeWidth={1.75} />
          </IconButton>
        </div>
        <button
          type="button"
          onClick={reset}
          disabled={!image}
          className="type-caption ml-auto h-[26px] rounded-control px-2 text-link hover:bg-surface-hover disabled:opacity-40"
        >
          Reset
        </button>
      </div>

      <div className="flex items-center gap-2">
        <div role="radiogroup" aria-label="Shape" className="flex gap-1">
          {SHAPES.map((s) => {
            const on = s.id === shapeId;
            return (
              <button
                key={s.id}
                type="button"
                role="radio"
                aria-checked={on}
                onClick={() => pickShape(s.id)}
                className={`type-caption h-[26px] rounded-control px-2 whitespace-nowrap ${
                  on ? "bg-surface-selected text-link" : "bg-surface text-secondary"
                }`}
              >
                {s.label}
              </button>
            );
          })}
        </div>
        {shapeId === "passport" && image && (
          <span className="type-caption ml-auto truncate text-tertiary">{passportHint(box)}</span>
        )}
      </div>

      <div className="flex gap-2">
        <Button onClick={onBack}>Back</Button>
        <Button type="submit" variant="primary" disabled={!image} className="flex-1" autoFocus>
          Crop
        </Button>
      </div>
    </form>
  );
}

function IconButton({
  label,
  onClick,
  disabled,
  children,
}: {
  label: string;
  onClick: () => void;
  disabled?: boolean;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled}
      aria-label={label}
      title={label}
      className="flex size-[30px] items-center justify-center rounded-control text-secondary hover:bg-surface-hover disabled:opacity-40"
    >
      {children}
    </button>
  );
}
