// PDF → images: pick pages on thumbnails or by typing "1-3, 5" (both stay in sync), choose a
// format and a size named for what it's for (Screen / Standard / Print), each with a short description.
import { useEffect, useMemo, useState } from "react";
import { Check, ChevronLeft } from "lucide-react";
import { Button } from "../components/Button";
import { PageGrid } from "../components/PageGrid";
import { Segmented } from "../components/Segmented";
import { isAppError, pdfPageSizes } from "../lib/ipc";
import { SIZES, allPages, clickPage, formatPages, parsePages, pixels } from "../lib/pages";
import type { FileInfo, JobRequest, PdfPageSize } from "../lib/types";

const FORMAT_HINTS = {
  jpg: "Smaller files. Best for photos and scans.",
  png: "Sharpest text and lines. Bigger files.",
};

interface Props {
  file: FileInfo;
  /** Settings → Conversions → PDF image size (72 / 150 / 300). */
  defaultDpi?: number;
  onBack: () => void;
  onRun: (request: JobRequest) => void;
}

export function PdfPagesEditor({ file, defaultDpi = 150, onBack, onRun }: Props) {
  const [sizes, setSizes] = useState<PdfPageSize[] | null>(null);
  const [error, setError] = useState<string>();
  const [selected, setSelected] = useState<Set<number>>(new Set());
  const [anchor, setAnchor] = useState<number | null>(null);
  const [rangeDraft, setRangeDraft] = useState("");
  const [rangeError, setRangeError] = useState<string>();
  const [typing, setTyping] = useState(false);
  const [format, setFormat] = useState<"jpg" | "png">("jpg");
  const [dpi, setDpi] = useState(defaultDpi);
  const count = sizes?.length ?? 0;

  useEffect(() => {
    let live = true;
    pdfPageSizes(file.path)
      .then((s) => {
        if (!live) return;
        setSizes(s);
        setSelected(allPages(s.length));
      })
      .catch((err) => live && setError(isAppError(err) ? err.message : "This PDF couldn't be opened."));
    return () => {
      live = false;
    };
  }, [file.path]);

  // Keep the text in sync with clicks (but don't fight the user while they type).
  useEffect(() => {
    if (!typing) {
      setRangeDraft(formatPages(selected));
      setRangeError(undefined);
    }
  }, [selected, typing]);

  const onRangeChange = (text: string) => {
    setRangeDraft(text);
    const parsed = parsePages(text, count);
    if ("error" in parsed) {
      setRangeError(text.trim() ? parsed.error : undefined);
    } else {
      setRangeError(undefined);
      setSelected(new Set(parsed.pages));
    }
  };

  const firstSelected = useMemo(() => Math.min(...selected), [selected]);
  const samplePage = Number.isFinite(firstSelected) ? firstSelected : 1;
  const sample = sizes?.[samplePage - 1];

  const submit = (e: React.FormEvent) => {
    e.preventDefault();
    if (selected.size === 0 || !sizes) return;
    onRun({
      action: "pdf_to_images",
      format,
      dpi,
      pages: selected.size === count ? { kind: "all" } : { kind: "range", spec: formatPages(selected) },
    });
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
        <h2 className="type-title text-primary">PDF to images</h2>
        <span className="type-caption ml-auto text-tertiary tabular-nums">
          {sizes ? `${selected.size} of ${count} ${count === 1 ? "page" : "pages"} selected` : ""}
        </span>
      </div>

      {error ? (
        <div className="flex h-[300px] items-center justify-center rounded-card bg-surface">
          <p className="type-body text-secondary">{error}</p>
        </div>
      ) : (
        <div className="flex gap-4">
          <PageGrid
            path={file.path}
            sizes={sizes}
            look={(page) => {
              const on = selected.has(page);
              return {
                label: `Page ${page}`,
                pressed: on,
                ring: on,
                faded: !on,
                badge: on ? (
                  <span className="flex size-4 items-center justify-center rounded-pill bg-accent text-on-accent">
                    <Check size={11} strokeWidth={3} />
                  </span>
                ) : undefined,
              };
            }}
            onClick={(page, e) => {
              setTyping(false);
              setSelected((s) => clickPage(s, page, anchor, e.shiftKey));
              setAnchor(page);
            }}
          />

          <div className="flex w-[232px] shrink-0 flex-col gap-3">
            <section className="flex flex-col gap-1.5">
              <div className="flex items-center justify-between">
                <span className="type-caption text-secondary">Pages</span>
                <div className="flex gap-1">
                  <Chip onClick={() => (setTyping(false), setSelected(allPages(count)))}>All</Chip>
                  <Chip onClick={() => (setTyping(false), setSelected(new Set()))}>None</Chip>
                </div>
              </div>
              <input
                value={rangeDraft}
                onChange={(e) => onRangeChange(e.target.value)}
                onFocus={() => setTyping(true)}
                onBlur={() => setTyping(false)}
                placeholder="1-3, 5"
                aria-label="Page range"
                aria-invalid={Boolean(rangeError)}
                className={`type-body h-[30px] rounded-control bg-surface px-2 text-primary tabular-nums outline-none placeholder:text-tertiary focus:ring-[3px] ${
                  rangeError ? "ring-1 ring-danger focus:ring-danger" : "focus:ring-focus-ring"
                }`}
              />
              <span className={`type-caption ${rangeError ? "text-error" : "text-tertiary"}`}>
                {rangeError ?? "Click pages, Shift-click for a range, or type them."}
              </span>
            </section>

            <section className="flex flex-col gap-1.5">
              <span className="type-caption text-secondary">Format</span>
              <Segmented
                label="Format"
                value={format}
                choices={[
                  { value: "jpg", label: "JPG" },
                  { value: "png", label: "PNG" },
                ]}
                onChange={(v) => setFormat(v as "jpg" | "png")}
              />
              <span className="type-caption text-tertiary">{FORMAT_HINTS[format]}</span>
            </section>

            <section className="flex flex-col gap-1.5">
              <span className="type-caption text-secondary">Image size</span>
              <div role="radiogroup" aria-label="Image size" className="flex flex-col gap-1">
                {SIZES.map((option) => {
                  const on = option.dpi === dpi;
                  return (
                    <button
                      key={option.dpi}
                      type="button"
                      role="radio"
                      aria-checked={on}
                      onClick={() => setDpi(option.dpi)}
                      className={`flex flex-col items-start rounded-control px-2.5 py-1.5 text-left ${
                        on ? "bg-surface-selected inset-ring inset-ring-accent" : "bg-surface hover:bg-surface-hover"
                      }`}
                    >
                      <span className="flex w-full items-baseline justify-between gap-2">
                        <span className={`type-body-strong ${on ? "text-link" : "text-primary"}`}>{option.label}</span>
                        {sample && (
                          <span className="type-caption text-tertiary tabular-nums">
                            {pixels(sample.width, option.dpi)} × {pixels(sample.height, option.dpi)} px
                          </span>
                        )}
                      </span>
                      <span className="type-caption text-secondary">{option.hint}</span>
                    </button>
                  );
                })}
              </div>
            </section>
          </div>
        </div>
      )}

      <div className="flex gap-2">
        <Button onClick={onBack}>Back</Button>
        <Button type="submit" variant="primary" disabled={!sizes || selected.size === 0} className="flex-1" autoFocus>
          {selected.size === 0
            ? "Pick at least one page"
            : `Export ${selected.size} ${selected.size === 1 ? "page" : "pages"} as ${format.toUpperCase()}`}
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
