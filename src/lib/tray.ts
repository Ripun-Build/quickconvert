// Page tray (Combine into PDF): order, turns and left-out photos, and how each page will look.
// Pure, so it runs under `node --test`.
import type { FileInfo, JobRequest, Turn } from "./types";

export interface TrayItem {
  file: FileInfo;
  /** Clockwise turn on top of the photo's own orientation. */
  turn: Turn;
  /** Left out of the PDF (greyed in place; click to bring back). */
  out: boolean;
}

export type TrayPageSize = "fit" | "a4" | "letter";
export type TrayMargin = "none" | "small";

export const fromFiles = (files: readonly FileInfo[]): TrayItem[] => files.map((file) => ({ file, turn: 0, out: false }));

/** Move the item at `from` so it lands just before the item now at `to` (`to` = length: the end). */
export function moveTo(items: readonly TrayItem[], from: number, to: number): TrayItem[] {
  const next = [...items];
  const [item] = next.splice(from, 1);
  next.splice(to > from ? to - 1 : to, 0, item);
  return next;
}

/** One place earlier (-1) or later (+1); unchanged at the ends. */
export function nudge(items: readonly TrayItem[], index: number, step: -1 | 1): TrayItem[] {
  const target = index + step;
  if (target < 0 || target >= items.length) return [...items];
  return moveTo(items, index, step > 0 ? index + 2 : index - 1);
}

export function toggleOut(items: readonly TrayItem[], index: number): TrayItem[] {
  return items.map((item, i) => (i === index ? { ...item, out: !item.out } : item));
}

/** A quarter turn right (+1) or left (-1). */
export function rotateItem(items: readonly TrayItem[], index: number, step: -1 | 1): TrayItem[] {
  return items.map((item, i) => (i === index ? { ...item, turn: ((((item.turn + step * 90) % 360) + 360) % 360) as Turn } : item));
}

/** Natural name order: IMG_2 before IMG_10. */
export function sortByName(items: readonly TrayItem[]): TrayItem[] {
  return [...items].sort((a, b) => a.file.name.localeCompare(b.file.name, undefined, { numeric: true, sensitivity: "base" }));
}

export const pageCount = (items: readonly TrayItem[]) => items.filter((i) => !i.out).length;

/** The job: photos still in the tray, in tray order, with their turns. */
export function trayRequest(
  items: readonly TrayItem[],
  pageSize: TrayPageSize,
  margin: TrayMargin,
): { request: JobRequest; files: FileInfo[] } {
  const kept = items.filter((i) => !i.out);
  const turns: Record<string, Turn> = {};
  for (const item of kept) if (item.turn !== 0) turns[item.file.path] = item.turn;
  return { request: { action: "images_to_pdf", pageSize, margin, turns }, files: kept.map((i) => i.file) };
}

// ---- Page preview (mirrors `page_layout` in src-tauri/src/convert/pdf.rs) ----

const A4: [number, number] = [595.276, 841.89];
const LETTER: [number, number] = [612, 792];
/** Small margin: 10 mm in points. */
const SMALL_MARGIN = (10 * 72) / 25.4;
/** Fit pages are the photo's own size, so a 10 mm margin is tiny on screen: show it as 2 %. */
const FIT_MARGIN = 0.02;

export interface PagePreview {
  /** Page width ÷ height. */
  aspect: number;
  /** Where the photo sits, as fractions of the page. */
  box: { x: number; y: number; w: number; h: number };
}

export function pagePreview(
  photo: { width: number; height: number },
  turn: Turn,
  pageSize: TrayPageSize,
  margin: TrayMargin,
): PagePreview {
  const sideways = turn === 90 || turn === 270;
  const w = sideways ? photo.height : photo.width;
  const h = sideways ? photo.width : photo.height;
  if (pageSize === "fit") {
    const m = margin === "small" ? FIT_MARGIN * Math.max(w, h) : 0;
    const pageW = w + 2 * m;
    const pageH = h + 2 * m;
    return { aspect: pageW / pageH, box: { x: m / pageW, y: m / pageH, w: w / pageW, h: h / pageH } };
  }
  const paper = pageSize === "a4" ? A4 : LETTER;
  // Landscape paper for landscape photos.
  const [pageW, pageH] = w > h ? [paper[1], paper[0]] : paper;
  const m = margin === "small" ? SMALL_MARGIN : 0;
  const scale = Math.min((pageW - 2 * m) / w, (pageH - 2 * m) / h);
  const bw = w * scale;
  const bh = h * scale;
  return {
    aspect: pageW / pageH,
    box: { x: (pageW - bw) / 2 / pageW, y: (pageH - bh) / 2 / pageH, w: bw / pageW, h: bh / pageH },
  };
}
