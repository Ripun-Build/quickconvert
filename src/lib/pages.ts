// Page selection for the PDF → images editor. Pure module so it runs under `node --test`.

/** Friendly names for render resolutions, so nobody has to know what DPI means. */
export const SIZES = [
  { dpi: 72, label: "Screen", hint: "For viewing and sharing. Smallest files." },
  { dpi: 150, label: "Standard", hint: "Good for most uses, like forms and email." },
  { dpi: 300, label: "Print", hint: "Sharpest, for printing. Largest files." },
] as const;

/** Pixel size of a page (in points) rendered at `dpi`. */
export const pixels = (points: number, dpi: number) => Math.max(1, Math.round((points * dpi) / 72));

/** Sorted, de-duplicated page numbers → "1-3, 5". */
export function formatPages(pages: Iterable<number>): string {
  const sorted = [...new Set(pages)].sort((a, b) => a - b);
  const parts: string[] = [];
  for (let i = 0; i < sorted.length; i++) {
    const start = sorted[i];
    while (i + 1 < sorted.length && sorted[i + 1] === sorted[i] + 1) i++;
    parts.push(start === sorted[i] ? `${start}` : `${start}-${sorted[i]}`);
  }
  return parts.join(", ");
}

/** "1-3, 5" → [1, 2, 3, 5], or a plain-language error. Mirrors `parse_pages` in Rust. */
export function parsePages(spec: string, pageCount: number): { pages: number[] } | { error: string } {
  const pages = new Set<number>();
  const parts = spec.split(",").map((p) => p.trim()).filter(Boolean);
  if (parts.length === 0) return { error: "Pick at least one page." };
  for (const part of parts) {
    const match = /^(\d+)\s*(?:-\s*(\d+))?$/.exec(part);
    if (!match) return { error: "Write pages like 1-3, 5." };
    const start = Number(match[1]);
    const end = Number(match[2] ?? match[1]);
    if (start === 0 || end < start) return { error: "Write pages like 1-3, 5." };
    if (end > pageCount) return { error: `This PDF has only ${pageCount} ${pageCount === 1 ? "page" : "pages"}.` };
    for (let p = start; p <= end; p++) pages.add(p);
  }
  return { pages: [...pages].sort((a, b) => a - b) };
}

/**
 * Click on a page tile: toggles it. Shift-click sets every page between the last clicked page
 * (`anchor`) and this one to the anchor's new state, like Finder.
 */
export function clickPage(selected: ReadonlySet<number>, page: number, anchor: number | null, shift: boolean): Set<number> {
  const next = new Set(selected);
  if (shift && anchor !== null) {
    const on = selected.has(anchor);
    const [from, to] = anchor < page ? [anchor, page] : [page, anchor];
    for (let p = from; p <= to; p++) {
      if (on) next.add(p);
      else next.delete(p);
    }
    return next;
  }
  if (next.has(page)) next.delete(page);
  else next.add(page);
  return next;
}

export const allPages = (count: number) => new Set(Array.from({ length: count }, (_, i) => i + 1));
