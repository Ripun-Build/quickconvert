// Split PDF: where new parts start, and what that makes. Pure, so it runs under `node --test`.

/** Pages that start a new part (page 1 always starts part 1 and is never listed). */
export type Starts = ReadonlySet<number>;

/** Click on a page: it starts a new part, or stops starting one. Page 1 always starts part 1. */
export function toggleStart(starts: Starts, page: number): Set<number> {
  const next = new Set(starts);
  if (page <= 1) return next;
  if (next.has(page)) next.delete(page);
  else next.add(page);
  return next;
}

/** A new part every `n` pages ("Every page" is n = 1). */
export function everyN(count: number, n: number): Set<number> {
  const starts = new Set<number>();
  for (let page = 1 + n; page <= count; page += n) starts.add(page);
  return starts;
}

/** The parts as lists of pages, in order. */
export function parts(count: number, starts: Starts): number[][] {
  const out: number[][] = [];
  for (let page = 1; page <= count; page++) {
    if (page === 1 || starts.has(page)) out.push([]);
    out[out.length - 1].push(page);
  }
  return out;
}

/** Which part (1-based) each page is in. */
export function partOf(count: number, starts: Starts): number[] {
  const out: number[] = [];
  let part = 0;
  for (let page = 1; page <= count; page++) {
    if (page === 1 || starts.has(page)) part++;
    out.push(part);
  }
  return out;
}

/** "pages 4–7", "page 3". */
export function describePart(pages: readonly number[]): string {
  return pages.length === 1 ? `page ${pages[0]}` : `pages ${pages[0]}–${pages[pages.length - 1]}`;
}
