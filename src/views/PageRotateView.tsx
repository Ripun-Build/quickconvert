// Rotate pages: like image Rotate, for a PDF's pages. Pick how every page should end up (Straight
// by default); each page is checked for which way is up and turned by what it needs. Click a page
// to turn it yourself. Only the page's rotation changes, never its content.
import { useEffect, useState } from "react";
import { Check as CheckIcon, ChevronLeft, CircleHelp, RotateCw } from "lucide-react";
import { Button } from "../components/Button";
import { PageGrid, type PageLook } from "../components/PageGrid";
import { Segmented } from "../components/Segmented";
import { isAppError, pdfPageSizes, pdfPageUpright } from "../lib/ipc";
import { CHECKING, TARGETS, describeTurn, planTurns, summary, turnFor, type Check } from "../lib/rotate";
import type { FileInfo, JobRequest, PdfPageSize, Turn } from "../lib/types";

/** Pages checked at the same time (each asks the system's Vision framework). */
const PARALLEL = 4;

interface Props {
  file: FileInfo;
  onBack: () => void;
  onRun: (request: JobRequest) => void;
}

export function PageRotateView({ file, onBack, onRun }: Props) {
  const [sizes, setSizes] = useState<PdfPageSize[] | null>(null);
  const [error, setError] = useState<string>();
  const [target, setTarget] = useState("0");
  const [checks, setChecks] = useState<Record<string, Check>>({});
  const count = sizes?.length ?? 0;

  useEffect(() => {
    let live = true;
    pdfPageSizes(file.path)
      .then((s) => {
        if (!live) return;
        setSizes(s);
        const queue = s.map((_, i) => i + 1);
        const settle = (page: number, upright: Turn | null) =>
          setChecks((c) => ({ ...c, [page]: { checking: false, upright, extra: c[page]?.extra ?? 0 } }));
        const worker = async () => {
          for (let page = queue.shift(); page; page = queue.shift()) {
            const p = page;
            try {
              const upright = await pdfPageUpright(file.path, p);
              if (live) settle(p, upright);
            } catch {
              if (live) settle(p, null);
            }
          }
        };
        for (let i = 0; i < PARALLEL; i++) void worker();
      })
      .catch((err) => live && setError(isAppError(err) ? err.message : "This PDF couldn't be opened."));
    return () => {
      live = false;
    };
  }, [file.path]);

  const targetTurn = Number(target) as Turn;
  const keys = Array.from({ length: count }, (_, i) => String(i + 1));
  const p = planTurns(keys, checks, targetTurn);
  const checking = p.checking > 0 || !sizes;

  const turnBy = (page: number, step: number) =>
    setChecks((c) => {
      const current = c[page] ?? CHECKING;
      return { ...c, [page]: { ...current, extra: current.extra + step } };
    });

  const look = (page: number): PageLook => {
    const check = checks[page] ?? CHECKING;
    const turn = turnFor(check, targetTurn);
    const unsure = !check.checking && check.upright === null && check.extra === 0;
    const status = check.checking
      ? "checking which way is up"
      : unsure
        ? "couldn't tell which way is up, left as it is"
        : turn === 0
          ? "already right"
          : `will be turned ${describeTurn(turn)}`;
    const Badge = unsure ? CircleHelp : turn === 0 ? CheckIcon : RotateCw;
    const colour = unsure ? "text-caution" : turn === 0 ? "text-tertiary" : "text-link";
    return {
      label: `Page ${page}: ${status}. Click to turn right, Shift-click to turn left.`,
      turn,
      faded: !check.checking && !unsure && turn === 0,
      badge: check.checking ? undefined : (
        <span className="flex size-[18px] items-center justify-center rounded-pill bg-glass shadow-control">
          <Badge size={12} strokeWidth={2} className={colour} />
        </span>
      ),
    };
  };

  const label = checking
    ? "Checking…"
    : p.keys.length === 0
      ? "Nothing to turn"
      : `Rotate ${p.keys.length} ${p.keys.length === 1 ? "page" : "pages"}`;

  return (
    <form
      onSubmit={(e) => {
        e.preventDefault();
        if (!checking && p.keys.length > 0) onRun({ action: "pdf_rotate", turns: p.turns });
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
        <h2 className="type-title text-primary">Rotate pages</h2>
      </div>

      <div className="flex items-center gap-3">
        <span className="type-caption shrink-0 text-secondary">Make every page</span>
        <div className="flex-1">
          <Segmented label="Make every page" value={target} choices={TARGETS} onChange={setTarget} />
        </div>
      </div>

      {error ? (
        <div className="flex h-[300px] items-center justify-center rounded-card bg-surface">
          <p className="type-body text-secondary">{error}</p>
        </div>
      ) : (
        <PageGrid
          path={file.path}
          sizes={sizes}
          look={look}
          square
          onClick={(page, e) => turnBy(page, e.shiftKey ? -1 : 1)}
          onKeyDown={(page, e) => {
            if (e.key === "ArrowRight" || e.key === "ArrowLeft") {
              e.preventDefault();
              turnBy(page, e.key === "ArrowRight" ? 1 : -1);
            }
          }}
        />
      )}

      <p className="type-caption min-h-[14px] text-tertiary" aria-live="polite">
        {sizes ? summary(p, count) : ""}
      </p>

      <div className="flex gap-2">
        <Button onClick={onBack}>Back</Button>
        <Button
          type="submit"
          variant="primary"
          disabled={!checking && p.keys.length === 0}
          aria-disabled={checking}
          className={`flex-1 ${checking ? "opacity-40" : ""}`}
          autoFocus
        >
          {label}
        </Button>
      </div>
    </form>
  );
}
