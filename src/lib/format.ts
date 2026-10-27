// Display formatting for file names, sizes and counts.
import type { Engine, FileInfo, FontSwap, Removed } from "./types";

/** Decimal units, matching Finder: 2.4 MB, 38 MB, 512 KB. */
export function formatBytes(bytes: number): string {
  if (bytes < 1000) return `${bytes} B`;
  const units = ["KB", "MB", "GB"];
  let value = bytes / 1000;
  let unit = 0;
  while (value >= 1000 && unit < units.length - 1) {
    value /= 1000;
    unit++;
  }
  const digits = value < 10 ? 1 : 0;
  return `${value.toFixed(digits)} ${units[unit]}`;
}

/** Keep the start and the extension: ("IMG_20240412_093011_0412.jpg", 20) → "IMG_202404…_0412.jpg". */
export function truncateMiddle(name: string, max = 28): string {
  if (name.length <= max) return name;
  const keepEnd = Math.min(10, Math.floor((max - 1) / 2));
  const keepStart = max - 1 - keepEnd;
  return `${name.slice(0, keepStart)}…${name.slice(-keepEnd)}`;
}

/** "12 images", "3 PDFs", "5 files". */
export function describeCount(files: readonly FileInfo[]): string {
  const n = files.length;
  const images = files.filter((f) => f.kind === "image").length;
  if (images === n) return `${n} ${n === 1 ? "image" : "images"}`;
  if (images === 0) return `${n} ${n === 1 ? "PDF" : "PDFs"}`;
  return `${n} files`;
}

export const totalSize = (files: readonly FileInfo[]) => files.reduce((sum, f) => sum + f.size, 0);

/** "48 KB · 200×230 · quality 78" (parts that are known). */
export function describeOutput(o: { bytes: number; width?: number; height?: number; quality?: number | null }): string {
  const parts = [formatBytes(o.bytes)];
  if (o.width && o.height) parts.push(`${o.width}×${o.height}`);
  if (o.quality) parts.push(`quality ${o.quality}`);
  return parts.join(" · ");
}

/** What a preset does, e.g. "200 × 230 px · 20–50 KB · JPG · 200 DPI". */
export function presetSummary(p: {
  format: string;
  width?: number;
  height?: number;
  aspect?: string;
  longEdge?: number;
  minKB?: number;
  maxKB?: number;
  dpi?: number;
}): string {
  const parts: string[] = [];
  if (p.width && p.height) parts.push(`${p.width} × ${p.height} px`);
  else if (p.aspect) parts.push(`${p.aspect} crop`);
  if (p.longEdge && !(p.width && p.height)) parts.push(`${p.longEdge} px long edge`);
  if (p.minKB && p.maxKB) parts.push(`${p.minKB}–${p.maxKB} KB`);
  else if (p.maxKB) parts.push(`under ${p.maxKB >= 1000 && p.maxKB % 1000 === 0 ? `${p.maxKB / 1000} MB` : `${p.maxKB} KB`}`);
  else if (p.minKB) parts.push(`at least ${p.minKB} KB`);
  parts.push(p.format.toUpperCase());
  if (p.dpi) parts.push(`${p.dpi} DPI`);
  return parts.join(" · ");
}

/** "a, b and c" */
const listJoin = (items: string[]) =>
  items.length <= 1 ? (items[0] ?? "") : `${items.slice(0, -1).join(", ")} and ${items[items.length - 1]}`;

/**
 * What Remove info took out, e.g. "Removed location, camera and date info." or, for a batch,
 * "Removed location and camera info (3 of 5 images had some)." Undefined for other actions.
 */
export function describeRemoved(outputs: readonly { removed?: Removed }[]): string | undefined {
  const found = outputs.flatMap((o) => (o.removed ? [o.removed] : []));
  if (found.length === 0) return undefined;
  const had = found.filter((r) => r.location || r.camera || r.date || r.other).length;
  if (had === 0) {
    return found.length === 1
      ? "No location or camera info found. Saved a clean copy anyway."
      : "No location or camera info found in these images.";
  }
  const any = (key: keyof Removed) => found.some((r) => r[key]);
  const kinds = (["location", "camera", "date"] as const).filter(any);
  const what = kinds.length === 0 ? "hidden info" : `${listJoin([...kinds])} info${any("other") ? " and other details" : ""}`;
  if (found.length === 1) return `Removed ${what}.`;
  return had === found.length
    ? `Removed ${what} from all ${found.length} images.`
    : `Removed ${what} (${had} of ${found.length} images had some).`;
}

const MADE_WITH: Record<Engine, string> = {
  quick: "Quick (built in)",
  pages: "Pages",
  keynote: "Keynote",
  numbers: "Numbers",
  libreoffice: "LibreOffice",
};

/** Convert to PDF: "Fonts not on this Mac were replaced with similar ones: Univers → Helvetica Neue."
 * Each swap once across all files; after four, "and 3 more". Undefined when nothing was swapped. */
export function describeFonts(outputs: readonly { fontsReplaced?: FontSwap[] }[]): string | undefined {
  const swaps = [...new Set(outputs.flatMap((o) => o.fontsReplaced ?? []).map((s) => `${s.from} → ${s.to}`))];
  if (swaps.length === 0) return undefined;
  const shown = swaps.slice(0, 4).join(", ");
  const more = swaps.length > 4 ? ` and ${swaps.length - 4} more` : "";
  return `Fonts not on this Mac were replaced with similar ones: ${shown}${more}.`;
}

/** Convert to PDF: "Made with Keynote", or a note that Quick isn't an exact copy. Undefined otherwise. */
export function describeMadeWith(outputs: readonly { madeWith?: Engine }[]): string | undefined {
  const engines = [...new Set(outputs.flatMap((o) => (o.madeWith ? [o.madeWith] : [])))];
  if (engines.length === 0) return undefined;
  const names = listJoin(engines.map((e) => MADE_WITH[e]));
  return engines.includes("quick") ? `Made with ${names}. Quick copies are close to the original, not 100% the same.` : `Made with ${names}.`;
}
