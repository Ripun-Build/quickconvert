// A PDF's pages as thumbnails in a scrolling 4-column grid (PDF → images, Split PDF, Rotate pages).
// Thumbnails load in batches as their tiles scroll into view; each view decides how a tile looks.
import { useCallback, useEffect, useRef, useState } from "react";
import { motion } from "framer-motion";
import { pdfThumbnails } from "../lib/ipc";
import { nearestAngle } from "../lib/rotate";
import type { PdfPageSize, Turn } from "../lib/types";

const THUMB_BATCH = 12;
const TURN = { type: "spring", stiffness: 300, damping: 30 } as const;

export interface PageLook {
  /** Accessible name, e.g. "Page 3, picked". */
  label: string;
  /** Accent ring (picked, starts a part). */
  ring?: boolean;
  /** Faded (not picked, already right). */
  faded?: boolean;
  /** Corner badge. */
  badge?: React.ReactNode;
  /** Line under the tile; the page number by default. */
  caption?: string;
  /** Show the page turned this way (clockwise), animating the short way round. */
  turn?: Turn;
  /** For toggle-style tiles. */
  pressed?: boolean;
}

interface Props {
  path: string;
  sizes: PdfPageSize[] | null;
  look: (page: number) => PageLook;
  onClick: (page: number, e: React.MouseEvent) => void;
  onKeyDown?: (page: number, e: React.KeyboardEvent) => void;
  /** Square cells, so a page can turn without leaving its cell (Rotate pages). */
  square?: boolean;
}

export function PageGrid({ path, sizes, look, onClick, onKeyDown, square = false }: Props) {
  const [thumbs, setThumbs] = useState<Record<number, string>>({});
  const requested = useRef(new Set<number>());
  const queue = useRef<number[]>([]);
  const busy = useRef(false);
  const rootRef = useRef<HTMLDivElement>(null);

  const pump = useCallback(() => {
    if (busy.current || queue.current.length === 0) return;
    const batch = queue.current.splice(0, THUMB_BATCH);
    busy.current = true;
    pdfThumbnails(path, batch)
      .then((urls) => setThumbs((t) => ({ ...t, ...Object.fromEntries(batch.map((p, i) => [p, urls[i]])) })))
      .catch(() => {})
      .finally(() => {
        busy.current = false;
        pump();
      });
  }, [path]);

  useEffect(() => {
    const root = rootRef.current;
    if (!root || !sizes) return;
    const observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          const page = Number((entry.target as HTMLElement).dataset.page);
          if (entry.isIntersecting && !requested.current.has(page)) {
            requested.current.add(page);
            queue.current.push(page);
          }
        }
        queue.current.sort((a, b) => a - b);
        pump();
      },
      { root, rootMargin: "200px" },
    );
    root.querySelectorAll("[data-page]").forEach((el) => observer.observe(el));
    return () => observer.disconnect();
  }, [sizes, pump]);

  const cell = square ? "aspect-square" : "aspect-[3/4]";
  return (
    <div ref={rootRef} className="h-[300px] min-w-0 flex-1 overflow-y-auto rounded-card bg-surface p-2">
      <div className="grid grid-cols-4 gap-2">
        {!sizes
          ? [0, 1, 2, 3, 4, 5, 6, 7].map((i) => <div key={i} className={`skeleton ${cell} rounded-control`} />)
          : sizes.map((size, i) => (
              <Tile
                key={i + 1}
                page={i + 1}
                size={size}
                thumb={thumbs[i + 1]}
                look={look(i + 1)}
                cell={cell}
                onClick={onClick}
                onKeyDown={onKeyDown}
              />
            ))}
      </div>
    </div>
  );
}

function Tile({
  page,
  size,
  thumb,
  look,
  cell,
  onClick,
  onKeyDown,
}: {
  page: number;
  size: PdfPageSize;
  thumb?: string;
  look: PageLook;
  cell: string;
  onClick: Props["onClick"];
  onKeyDown?: Props["onKeyDown"];
}) {
  const turn = look.turn ?? 0;
  const [angle, setAngle] = useState<number>(turn);
  useEffect(() => setAngle((a) => nearestAngle(a, turn)), [turn]);
  const tall = size.height >= size.width;

  return (
    <button
      type="button"
      data-page={page}
      aria-pressed={look.pressed}
      aria-label={look.label}
      title={look.label}
      onClick={(e) => onClick(page, e)}
      onKeyDown={onKeyDown && ((e) => onKeyDown(page, e))}
      className="group flex flex-col items-center gap-1 rounded-control p-1 hover:bg-surface-hover"
    >
      <div className={`relative flex ${cell} w-full items-center justify-center`}>
        <motion.div
          className={`relative max-h-full max-w-full overflow-hidden rounded-kbd bg-surface-raised shadow-control transition-opacity ${
            look.ring ? "ring-2 ring-accent" : ""
          } ${look.faded ? "opacity-55" : ""}`}
          style={{ aspectRatio: `${size.width} / ${size.height}`, height: tall ? "100%" : undefined, width: tall ? undefined : "100%" }}
          initial={false}
          animate={{ rotate: angle }}
          transition={TURN}
        >
          {thumb && <img src={thumb} alt="" draggable={false} className="size-full object-cover" />}
        </motion.div>
        {look.badge && <span className="absolute top-0.5 right-0.5">{look.badge}</span>}
      </div>
      <span className={`type-caption tabular-nums ${look.faded ? "text-tertiary" : "text-primary"}`}>
        {look.caption ?? page}
      </span>
    </button>
  );
}
