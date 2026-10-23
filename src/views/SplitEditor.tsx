// Split PDF: click the pages where new parts start (or use a quick choice) and get one PDF per
// part; or switch to "Keep pages" and pick the pages for a single new PDF.
import { useEffect, useMemo, useState } from "react";
import { Check, ChevronLeft, Scissors } from "lucide-react";
import { Button } from "../components/Button";
import { PageGrid, type PageLook } from "../components/PageGrid";
import { Segmented } from "../components/Segmented";
import { isAppError, pdfPageSizes } from "../lib/ipc";
import { allPages, clickPage, formatPages, parsePages } from "../lib/pages";
import { describePart, everyN, partOf, parts, toggleStart } from "../lib/split";
import type { FileInfo, JobRequest, PdfPageSize } from "../lib/types";

type Mode = "split" | "keep";
const QUICK = [
  { n: 1, label: "Every page" },
  { n: 2, label: "Every 2" },
  { n: 3, label: "Every 3" },
];
/** Parts listed before "+ N more". */
const LISTED = 6;

interface Props {
  file: FileInfo;
  onBack: () => void;
  onRun: (request: JobRequest) => void;
}

export function SplitEditor({ file, onBack, onRun }: Props) {
  const [sizes, setSizes] = useState<PdfPageSize[] | null>(null);
  const [error, setError] = useState<string>();
  const [mode, setMode] = useState<Mode>("split");
  const [starts, setStarts] = useState<Set<number>>(new Set());
  const [picked, setPicked] = useState<Set<number>>(new Set());
  const [anchor, setAnchor] = useState<number | null>(null);
  const [rangeDraft, setRangeDraft] = useState("");
  const [rangeError, setRangeError] = useState<string>();
  const [typing, setTyping] = useState(false);
  const count = sizes?.length ?? 0;

  useEffect(() => {
    let live = true;
    pdfPageSizes(file.path)
      .then((s) => live && setSizes(s))
      .catch((err) => live && setError(isAppError(err) ? err.message : "This PDF couldn't be opened."));
    return () => {
      live = false;
    };
  }, [file.path]);

  useEffect(() => {
    if (!typing) {
      setRangeDraft(formatPages(picked));
      setRangeError(undefined);
    }
  }, [picked, typing]);

  const split = useMemo(() => parts(count, starts), [count, starts]);
  const owner = useMemo(() => partOf(count, starts), [count, starts]);

  const look = (page: number): PageLook => {
    if (mode === "keep") {
      const on = picked.has(page);
      return {
        label: `Page ${page}${on ? ", kept" : ""}`,
        pressed: on,
        ring: on,
        faded: !on,
        badge: on ? (
          <span className="flex size-4 items-center justify-center rounded-pill bg-accent text-on-accent">
            <Check size={11} strokeWidth={3} />
          </span>
        ) : undefined,
      };
    }
    const part = owner[page - 1];
    const startsHere = page > 1 && starts.has(page);
    return {
      label:
        page === 1
          ? "Page 1, part 1 starts here"
          : `Page ${page}, part ${part}. ${startsHere ? "Click to join it to the part before" : "Click to start a new part here"}`,
      pressed: startsHere,
      ring: startsHere,
      caption: `${page} · Part ${part}`,
      badge: startsHere ? (
        <span className="flex size-4 items-center justify-center rounded-pill bg-accent text-on-accent">
          <Scissors size={10} strokeWidth={2.5} />
        </span>
      ) : undefined,
    };
  };

  const onPage = (page: number, e: React.MouseEvent) => {
    if (mode === "split") {
      setStarts((s) => toggleStart(s, page));
      return;
    }
    setTyping(false);
    setPicked((s) => clickPage(s, page, anchor, e.shiftKey));
    setAnchor(page);
  };

  const onRangeChange = (text: string) => {
    setRangeDraft(text);
    const parsed = parsePages(text, count);
    if ("error" in parsed) {
      setRangeError(text.trim() ? parsed.error : undefined);
    } else {
      setRangeError(undefined);
      setPicked(new Set(parsed.pages));
    }
  };

  const request: JobRequest | null =
    mode === "split"
      ? split.length >= 2
        ? { action: "pdf_split", parts: split }
        : null
      : picked.size > 0
        ? { action: "pdf_extract", pages: [...picked].sort((a, b) => a - b) }
        : null;

  const label =
    mode === "split"
      ? split.length >= 2
        ? `Split into ${split.length} PDFs`
        : "Click a page to start a new part"
      : picked.size > 0
        ? `Save ${picked.size} ${picked.size === 1 ? "page" : "pages"} as a new PDF`
        : "Click the pages to keep";

  return (
    <form
      onSubmit={(e) => {
        e.preventDefault();
        if (request) onRun(request);
      }}
      className="flex flex-col gap-3 px-4 pb-3"
    >
      <div className="flex items-center gap-1">
        <button
          type="button"
          onClick={onBack}
          aria-label="Back"
          className="-ml-1.5 flex size-7 items-center justify-center rounded-control text-secondary hover:bg-surface-hover"
        >
          <ChevronLeft size={18} strokeWidth={1.75} />
        </button>
        <h2 className="type-title text-primary">Split PDF</h2>
        <span className="type-caption ml-auto text-tertiary tabular-nums">
          {sizes ? `${count} ${count === 1 ? "page" : "pages"}` : ""}
        </span>
      </div>

      {error ? (
        <div className="flex h-[300px] items-center justify-center rounded-card bg-surface">
          <p className="type-body text-secondary">{error}</p>
        </div>
      ) : (
        <div className="flex gap-4">
          <PageGrid path={file.path} sizes={sizes} look={look} onClick={onPage} />

          <div className="flex w-[232px] shrink-0 flex-col gap-3">
            <Segmented
              label="What to do"
              value={mode}
              choices={[
                { value: "split", label: "Split into parts" },
                { value: "keep", label: "Keep pages" },
              ]}
              onChange={(v) => setMode(v as Mode)}
            />

            {mode === "split" ? (
              <>
                <section className="flex flex-col gap-1.5">
                  <span className="type-caption text-secondary">New part</span>
                  <div className="flex flex-wrap gap-1">
                    {QUICK.map((q) => (
                      <Chip key={q.n} onClick={() => setStarts(everyN(count, q.n))}>
                        {q.label}
                      </Chip>
                    ))}
                    <Chip onClick={() => setStarts(new Set())}>Clear</Chip>
                  </div>
                  <span className="type-caption text-tertiary">Click a page to start a new part there.</span>
                </section>
                <section className="flex flex-col gap-1" aria-live="polite">
                  <span className="type-caption text-secondary">
                    {split.length} {split.length === 1 ? "part" : "parts"}
                  </span>
                  {split.slice(0, LISTED).map((pages, i) => (
                    <span key={i} className="type-caption text-primary tabular-nums">
                      Part {i + 1} · {describePart(pages)}
                    </span>
                  ))}
                  {split.length > LISTED && (
                    <span className="type-caption text-tertiary">+ {split.length - LISTED} more</span>
                  )}
                </section>
              </>
            ) : (
              <section className="flex flex-col gap-1.5">
                <div className="flex items-center justify-between">
                  <span className="type-caption text-secondary">Pages to keep</span>
                  <div className="flex gap-1">
                    <Chip onClick={() => (setTyping(false), setPicked(allPages(count)))}>All</Chip>
                    <Chip onClick={() => (setTyping(false), setPicked(new Set()))}>None</Chip>
                  </div>
                </div>
                <input
                  value={rangeDraft}
                  onChange={(e) => onRangeChange(e.target.value)}
                  onFocus={() => setTyping(true)}
                  onBlur={() => setTyping(false)}
                  placeholder="2, 5-7"
                  aria-label="Pages to keep"
                  aria-invalid={Boolean(rangeError)}
                  className={`type-body h-[30px] rounded-control bg-surface px-2 text-primary tabular-nums outline-none placeholder:text-tertiary focus:ring-[3px] ${
                    rangeError ? "ring-1 ring-danger focus:ring-danger" : "focus:ring-focus-ring"
                  }`}
                />
                <span className={`type-caption ${rangeError ? "text-error" : "text-tertiary"}`}>
                  {rangeError ?? "Click pages, Shift-click for a range, or type them. They're saved as one new PDF."}
                </span>
              </section>
            )}
          </div>
        </div>
      )}

      <div className="flex gap-2">
        <Button onClick={onBack}>Back</Button>
        <Button type="submit" variant="primary" disabled={!request} className="flex-1" autoFocus>
          {label}
        </Button>
      </div>
    </form>
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
