// Display formatting for file names, sizes and counts.
import type { FileInfo } from "./types";

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

/** Keep the start and the extension: "IMG_20240412_093011_0412.jpg" → "IMG_20240…_0412.jpg". */
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
