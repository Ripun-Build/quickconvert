// Combine into PDF: arrange the pages first. Drag photos into order, turn them, leave some out, and
// pick the page size and margin; every tile previews the real page.
import { useEffect, useRef, useState } from "react";
import { motion } from "framer-motion";
import { ChevronLeft, RotateCw, Undo2, X } from "lucide-react";
import { Button } from "../components/Button";
import { Segmented } from "../components/Segmented";
import { rotateCheck, trayThumbnail } from "../lib/ipc";
import { nearestAngle } from "../lib/rotate";
import {
  fromFiles,
  moveTo,
  nudge,
  pageCount,
  pagePreview,
  rotateItem,
  sortByName,
  toggleOut,
  trayRequest,
  type TrayItem,
  type TrayMargin,
  type TrayPageSize,
} from "../lib/tray";
import type { FileInfo, JobRequest, Preview } from "../lib/types";

const TURN = { type: "spring", stiffness: 300, damping: 30 } as const;
/** Thumbnails / straighten checks running at once. */
const PARALLEL = 4;
/** Pointer travel (px) before a press becomes a drag. */
const DRAG_START = 5;
/** Distance from the grid's top/bottom edge (px) where dragging scrolls it. */
const EDGE = 32;

interface Drag {
  from: number;
  startX: number;
  startY: number;
  x: number;
  y: number;
  active: boolean;
  /** Drop position: before this index (length = at the end). */
  to: number | null;
}

interface Props {
  files: FileInfo[];
  onBack: () => void;
  /** `files`: the photos still in the tray, in tray order. */
  onRun: (request: JobRequest, files: FileInfo[]) => void;
}

export function PageTray({ files, onBack, onRun }: Props) {
  const [items, setItems] = useState<TrayItem[]>(() => fromFiles(files));
  const [pageSize, setPageSize] = useState<TrayPageSize>("fit");
  const [margin, setMargin] = useState<TrayMargin>("none");
  const [thumbs, setThumbs] = useState<Record<string, Preview>>({});
  const [drag, setDrag] = useState<Drag | null>(null);
  const [straight, setStraight] = useState<{ done: number; total: number; turned: number; unsure: number } | null>(null);
  const gridRef = useRef<HTMLDivElement>(null);
  const tileRefs = useRef<(HTMLDivElement | null)[]>([]);

  useEffect(() => {
    let live = true;
    const queue = files.map((f) => f.path);
    const worker = async () => {
      for (let path = queue.shift(); path; path = queue.shift()) {
        const p = path;
        try {
          const thumb = await trayThumbnail(p);
          if (live) setThumbs((t) => ({ ...t, [p]: thumb }));
        } catch {
          // keep the placeholder; the job reports unreadable files
        }
      }
    };
    for (let i = 0; i < PARALLEL; i++) void worker();
    return () => {
      live = false;
    };
  }, [files]);

  const pages = pageCount(items);

  // ---- Drag to reorder ----

  const dropIndex = (x: number, y: number, current: number | null) => {
    for (let k = 0; k < tileRefs.current.length; k++) {
      const rect = tileRefs.current[k]?.getBoundingClientRect();
      if (rect && x >= rect.left && x <= rect.right && y >= rect.top && y <= rect.bottom) {
        return x < rect.left + rect.width / 2 ? k : k + 1;
      }
    }
    return current;
  };

  const onPointerDown = (index: number) => (e: React.PointerEvent) => {
    if (e.button !== 0 || (e.target as HTMLElement).closest("button")) return;
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    setDrag({ from: index, startX: e.clientX, startY: e.clientY, x: e.clientX, y: e.clientY, active: false, to: null });
  };

  const onPointerMove = (e: React.PointerEvent) => {
    if (!drag) return;
    const active = drag.active || Math.hypot(e.clientX - drag.startX, e.clientY - drag.startY) > DRAG_START;
    if (!active) return;
    const grid = gridRef.current;
    if (grid) {
      const rect = grid.getBoundingClientRect();
      if (e.clientY < rect.top + EDGE) grid.scrollTop -= 12;
      else if (e.clientY > rect.bottom - EDGE) grid.scrollTop += 12;
    }
    setDrag({ ...drag, active, x: e.clientX, y: e.clientY, to: dropIndex(e.clientX, e.clientY, drag.to) });
  };

  const onPointerUp = () => {
    if (drag?.active && drag.to !== null) setItems((list) => moveTo(list, drag.from, drag.to ?? drag.from));
    setDrag(null);
  };

  // ---- Keyboard ----

  const onTileKey = (index: number) => (e: React.KeyboardEvent) => {
    if (e.altKey && (e.key === "ArrowLeft" || e.key === "ArrowRight")) {
      e.preventDefault();
      setItems((list) => nudge(list, index, e.key === "ArrowRight" ? 1 : -1));
    } else if (e.key.toLowerCase() === "r" && !e.metaKey && !e.ctrlKey) {
      e.preventDefault();
      setItems((list) => rotateItem(list, index, e.shiftKey ? -1 : 1));
    } else if (e.key === "Delete" || e.key === "Backspace") {
      e.preventDefault();
      setItems((list) => toggleOut(list, index));
    }
  };

  // ---- Straighten: the same which-way-is-up check as Rotate ----

  const straighten = () => {
    const queue = items.filter((i) => !i.out).map((i) => i.file.path);
    const total = queue.length;
    let done = 0;
    let turned = 0;
    let unsure = 0;
    setStraight({ done, total, turned, unsure });
    const worker = async () => {
      for (let path = queue.shift(); path; path = queue.shift()) {
        const p = path;
        try {
          const { upright } = await rotateCheck(p, 64);
          if (upright === null) unsure++;
          else {
            if (upright !== 0) turned++;
            setItems((list) => list.map((item) => (item.file.path === p ? { ...item, turn: upright } : item)));
          }
        } catch {
          unsure++;
        }
        done++;
        setStraight({ done, total, turned, unsure });
      }
    };
    for (let i = 0; i < PARALLEL; i++) void worker();
  };
  const straightening = straight !== null && straight.done < straight.total;

  const submit = (e: React.FormEvent) => {
    e.preventDefault();
    if (pages === 0) return;
    const { request, files: kept } = trayRequest(items, pageSize, margin);
    onRun(request, kept);
  };

  const left = items.length - pages;
  const dragged = drag?.active ? items[drag.from] : null;

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
        <h2 className="type-title text-primary">Combine into PDF</h2>
        <span className="type-caption ml-auto text-tertiary tabular-nums">
          {pages} {pages === 1 ? "page" : "pages"}
          {left > 0 ? ` · ${left} left out` : ""}
        </span>
      </div>

      <div className="flex gap-4">
        <div
          ref={gridRef}
          role="list"
          aria-label="Pages"
          onPointerMove={onPointerMove}
          onPointerUp={onPointerUp}
          onPointerCancel={() => setDrag(null)}
          className="h-[300px] min-w-0 flex-1 overflow-y-auto rounded-card bg-surface p-2"
        >
          <div className="grid grid-cols-4 gap-2">
            {items.map((item, index) => (
              <Tile
                key={item.file.path}
                tileRef={(el) => {
                  tileRefs.current[index] = el;
                }}
                item={item}
                thumb={thumbs[item.file.path]}
                pageNumber={item.out ? null : items.slice(0, index + 1).filter((i) => !i.out).length}
                pageSize={pageSize}
                margin={margin}
                lifted={drag?.active === true && drag.from === index}
                dropBefore={drag?.active === true && drag.to === index}
                dropAfter={drag?.active === true && drag.to === items.length && index === items.length - 1}
                onPointerDown={onPointerDown(index)}
                onKeyDown={onTileKey(index)}
                onRotate={(step) => setItems((list) => rotateItem(list, index, step))}
                onToggleOut={() => setItems((list) => toggleOut(list, index))}
              />
            ))}
          </div>
        </div>

        <div className="flex w-[232px] shrink-0 flex-col gap-3">
          <section className="flex flex-col gap-1.5">
            <span className="type-caption text-secondary">Page size</span>
            <Segmented
              label="Page size"
              value={pageSize}
              choices={[
                { value: "fit", label: "Fit" },
                { value: "a4", label: "A4" },
                { value: "letter", label: "Letter" },
              ]}
              onChange={(v) => setPageSize(v as TrayPageSize)}
            />
            <span className="type-caption text-tertiary">
              {pageSize === "fit" ? "Each page is the photo's own size." : "Wide photos get landscape pages."}
            </span>
          </section>
          <section className="flex flex-col gap-1.5">
            <span className="type-caption text-secondary">Margin</span>
            <Segmented
              label="Margin"
              value={margin}
              choices={[
                { value: "none", label: "None" },
                { value: "small", label: "Small" },
              ]}
              onChange={(v) => setMargin(v as TrayMargin)}
            />
          </section>
          <section className="flex flex-col gap-1.5">
            <span className="type-caption text-secondary">Order</span>
            <div className="flex flex-wrap gap-1">
              <Chip onClick={() => setItems((list) => sortByName(list))}>Name A→Z</Chip>
              <Chip onClick={() => setItems((list) => [...list].reverse())}>Reverse</Chip>
              <Chip onClick={() => setItems(fromFiles(files))}>Reset</Chip>
            </div>
            <span className="type-caption text-tertiary">Drag photos to move them. ⌥← ⌥→ move, R turns, Delete leaves out.</span>
          </section>
          <section className="flex flex-col gap-1.5" aria-live="polite">
            <Button onClick={straighten} disabled={straightening || pages === 0}>
              {straightening ? `Checking ${straight.done} of ${straight.total}…` : "Straighten photos"}
            </Button>
            {straight && !straightening && (
              <span className="type-caption text-tertiary">
                {straight.turned === 0 ? "All already straight" : `Turned ${straight.turned} ${straight.turned === 1 ? "photo" : "photos"}`}
                {straight.unsure > 0 ? ` · ${straight.unsure} couldn't tell` : ""}
              </span>
            )}
          </section>
        </div>
      </div>

      <div className="flex gap-2">
        <Button onClick={onBack}>Back</Button>
        <Button type="submit" variant="primary" disabled={pages === 0} className="flex-1" autoFocus>
          {pages === 0 ? "Bring back at least one photo" : `Create PDF · ${pages} ${pages === 1 ? "page" : "pages"}`}
          {left > 0 && pages > 0 && <span className="opacity-70">({left} left out)</span>}
        </Button>
      </div>

      {dragged && drag && (
        <div
          aria-hidden
          className="pointer-events-none fixed z-10 size-14 overflow-hidden rounded-kbd bg-surface-raised opacity-90 shadow-popup"
          style={{ left: drag.x - 28, top: drag.y - 28 }}
        >
          {thumbs[dragged.file.path] && (
            <img src={thumbs[dragged.file.path].url} alt="" draggable={false} className="size-full object-cover" />
          )}
        </div>
      )}
    </form>
  );
}

interface TileProps {
  /** Not `ref`: React 18 doesn't pass `ref` to function components as a prop. */
  tileRef: (el: HTMLDivElement | null) => void;
  item: TrayItem;
  thumb?: Preview;
  pageNumber: number | null;
  pageSize: TrayPageSize;
  margin: TrayMargin;
  lifted: boolean;
  dropBefore: boolean;
  dropAfter: boolean;
  onPointerDown: (e: React.PointerEvent) => void;
  onKeyDown: (e: React.KeyboardEvent) => void;
  onRotate: (step: -1 | 1) => void;
  onToggleOut: () => void;
}

/** One page: the photo placed on its page as it will come out. */
function Tile({
  tileRef,
  item,
  thumb,
  pageNumber,
  pageSize,
  margin,
  lifted,
  dropBefore,
  dropAfter,
  onPointerDown,
  onKeyDown,
  onRotate,
  onToggleOut,
}: TileProps) {
  const [angle, setAngle] = useState<number>(item.turn);
  useEffect(() => setAngle((a) => nearestAngle(a, item.turn)), [item.turn]);
  const photo = thumb ?? { width: 3, height: 4 };
  const page = pagePreview(photo, item.turn, pageSize, margin);
  const sideways = item.turn === 90 || item.turn === 270;
  // The picture inside its box: for a quarter turn its width spans the box's height.
  const shown = sideways ? photo.height / photo.width : photo.width / photo.height;
  const label = item.out
    ? `${item.file.name}, left out. Click or press Delete to bring it back.`
    : `${item.file.name}, page ${pageNumber}. Drag or press Option-arrows to move it, R to turn it, Delete to leave it out.`;

  return (
    <motion.div
      ref={tileRef}
      layout
      transition={TURN}
      role="listitem"
      tabIndex={0}
      aria-label={label}
      title={item.file.name}
      onPointerDown={onPointerDown}
      onKeyDown={onKeyDown}
      onClick={item.out ? onToggleOut : undefined}
      className={`group relative flex cursor-grab touch-none flex-col items-center gap-1 rounded-control p-1 hover:bg-surface-hover ${
        lifted ? "opacity-40" : ""
      }`}
    >
      {dropBefore && <span aria-hidden className="absolute inset-y-1 -left-1.5 w-[3px] rounded-pill bg-accent" />}
      {dropAfter && <span aria-hidden className="absolute inset-y-1 -right-1.5 w-[3px] rounded-pill bg-accent" />}
      <div className="relative flex aspect-square w-full items-center justify-center">
        <div
          className={`relative max-h-full max-w-full overflow-hidden rounded-kbd bg-surface-raised shadow-control transition-opacity ${
            item.out ? "opacity-35" : ""
          }`}
          style={{ aspectRatio: page.aspect, height: page.aspect <= 1 ? "100%" : undefined, width: page.aspect > 1 ? "100%" : undefined }}
        >
          <div
            className="absolute overflow-hidden"
            style={{
              left: `${page.box.x * 100}%`,
              top: `${page.box.y * 100}%`,
              width: `${page.box.w * 100}%`,
              height: `${page.box.h * 100}%`,
            }}
          >
            {thumb ? (
              <motion.img
                src={thumb.url}
                alt=""
                draggable={false}
                className="absolute top-1/2 left-1/2 max-w-none object-fill"
                style={{ x: "-50%", y: "-50%" }}
                initial={false}
                animate={{
                  rotate: angle,
                  width: sideways ? `${100 / shown}%` : "100%",
                  height: sideways ? `${100 * shown}%` : "100%",
                }}
                transition={TURN}
              />
            ) : (
              <div className="skeleton size-full" />
            )}
          </div>
        </div>
        <div className="absolute top-0.5 right-0.5 flex gap-0.5 opacity-0 transition-opacity group-hover:opacity-100 group-focus-within:opacity-100">
          {!item.out && (
            <TileButton label="Turn right (Shift-click: left)" onClick={(e) => onRotate(e.shiftKey ? -1 : 1)}>
              <RotateCw size={12} strokeWidth={2} />
            </TileButton>
          )}
          <TileButton label={item.out ? "Bring back" : "Leave out"} onClick={() => onToggleOut()}>
            {item.out ? <Undo2 size={12} strokeWidth={2} /> : <X size={12} strokeWidth={2} />}
          </TileButton>
        </div>
      </div>
      <span className={`type-caption tabular-nums ${item.out ? "text-tertiary" : "text-primary"}`}>
        {item.out ? "Left out" : `Page ${pageNumber}`}
      </span>
    </motion.div>
  );
}

function TileButton({
  label,
  onClick,
  children,
}: {
  label: string;
  onClick: (e: React.MouseEvent) => void;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      onClick={(e) => {
        e.stopPropagation();
        onClick(e);
      }}
      className="flex size-[22px] items-center justify-center rounded-pill bg-glass text-primary shadow-control hover:bg-surface-raised"
    >
      {children}
    </button>
  );
}

function Chip({ children, onClick }: { children: React.ReactNode; onClick: () => void }) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="type-caption h-[22px] rounded-control bg-surface px-2 text-secondary hover:bg-surface-hover"
    >
      {children}
    </button>
  );
}
