// Rotate: pick how every image should end up (Straight by default). QuickConvert checks which way
// is up for each image (its text and faces) and turns each one by what it needs; images already
// right are skipped. Click a picture to turn it yourself.
import { useEffect, useState } from "react";
import { motion } from "framer-motion";
import { Check as CheckIcon, ChevronLeft, CircleHelp, RotateCw } from "lucide-react";
import { Button } from "../components/Button";
import { Segmented } from "../components/Segmented";
import { rotateCheck } from "../lib/ipc";
import { CHECKING, TARGETS, describeTurn, nearestAngle, plan, summary, turnFor, type Check } from "../lib/rotate";
import type { FileInfo, JobRequest, Turn } from "../lib/types";

const TURN = { type: "spring", stiffness: 300, damping: 30 } as const;
/** Pictures checked at the same time (each one asks the system's Vision framework). */
const PARALLEL = 3;
/** Single image: one large picture. */
const ONE_SIZE = 176;

interface Props {
  files: FileInfo[];
  onBack: () => void;
  /** `files`: only the images that need turning. */
  onRun: (request: JobRequest, files: FileInfo[]) => void;
}

export function RotateView({ files, onBack, onRun }: Props) {
  const [target, setTarget] = useState("0");
  const [checks, setChecks] = useState<Record<string, Check>>({});
  const [urls, setUrls] = useState<Record<string, string>>({});
  const one = files.length === 1;

  useEffect(() => {
    let live = true;
    const queue = [...files];
    const size = one ? 512 : 192;
    const settle = (path: string, upright: Turn | null) =>
      setChecks((c) => ({ ...c, [path]: { checking: false, upright, extra: c[path]?.extra ?? 0 } }));
    const worker = async () => {
      for (let file = queue.shift(); file; file = queue.shift()) {
        const path = file.path;
        try {
          const result = await rotateCheck(path, size);
          if (!live) return;
          setUrls((u) => ({ ...u, [path]: result.url }));
          settle(path, result.upright);
        } catch {
          if (live) settle(path, null); // unreadable: shown as "couldn't tell"; the job reports it
        }
      }
    };
    for (let i = 0; i < PARALLEL; i++) void worker();
    return () => {
      live = false;
    };
  }, [files, one]);

  const targetTurn = Number(target) as Turn;
  const p = plan(files, checks, targetTurn);
  const checking = p.checking > 0;

  const turnBy = (path: string, step: number) =>
    setChecks((c) => {
      const current = c[path] ?? CHECKING;
      return { ...c, [path]: { ...current, extra: current.extra + step } };
    });

  const label = checking
    ? "Checking…"
    : p.files.length === 0
      ? "Nothing to turn"
      : one
        ? "Rotate"
        : p.files.length < files.length
          ? `Rotate ${p.files.length} of ${files.length} images`
          : `Rotate ${files.length} images`;

  return (
    <form
      onSubmit={(e) => {
        e.preventDefault();
        if (!checking && p.files.length > 0) onRun(p.request, p.files);
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
        <h2 className="type-title text-primary">Rotate</h2>
      </div>

      <div className="flex flex-col gap-1.5">
        <span className="type-caption text-secondary">{one ? "Make it" : "Make every image"}</span>
        <Segmented label={one ? "Make it" : "Make every image"} value={target} choices={TARGETS} onChange={setTarget} />
      </div>

      {one ? (
        <div className="flex justify-center rounded-card bg-surface py-2">
          <Picture
            file={files[0]}
            url={urls[files[0].path]}
            check={checks[files[0].path] ?? CHECKING}
            target={targetTurn}
            onTurn={(step) => turnBy(files[0].path, step)}
            style={{ width: ONE_SIZE, height: ONE_SIZE }}
          />
        </div>
      ) : (
        <div role="group" aria-label="Images" className="scroll-fade grid max-h-[208px] grid-cols-4 gap-2 overflow-y-auto py-1">
          {files.map((file) => (
            <Picture
              key={file.path}
              file={file}
              url={urls[file.path]}
              check={checks[file.path] ?? CHECKING}
              target={targetTurn}
              onTurn={(step) => turnBy(file.path, step)}
              className="aspect-square w-full"
            />
          ))}
        </div>
      )}

      <p className="type-caption min-h-[14px] text-tertiary" aria-live="polite">
        {one ? oneSummary(p, checks[files[0].path] ?? CHECKING, targetTurn) : summary(p, files.length)}
      </p>

      <div className="flex gap-2">
        <Button onClick={onBack}>Back</Button>
        <Button
          type="submit"
          variant="primary"
          disabled={!checking && p.files.length === 0}
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

function oneSummary(p: ReturnType<typeof plan>, check: Check, target: Turn): string {
  if (check.checking) return "Checking which way is up…";
  const turn = turnFor(check, target);
  if (p.unknown > 0) return "Couldn't tell which way is up. Click the picture to turn it.";
  return turn === 0 ? "Already the way you picked. Nothing to turn." : `Will be turned ${describeTurn(turn)}.`;
}

interface PictureProps {
  file: FileInfo;
  url?: string;
  check: Check;
  target: Turn;
  /** +1 = a quarter turn right, −1 = left. */
  onTurn: (step: number) => void;
  className?: string;
  style?: React.CSSProperties;
}

/** One image as it will come out. Click: turn right; Shift-click or ←: turn left. */
function Picture({ file, url, check, target, onTurn, className = "", style }: PictureProps) {
  const turn = turnFor(check, target);
  const [angle, setAngle] = useState<number>(turn);
  useEffect(() => setAngle((a) => nearestAngle(a, turn)), [turn]);

  const unsure = !check.checking && check.upright === null && check.extra === 0;
  const status = check.checking
    ? "checking which way is up"
    : unsure
      ? "couldn't tell which way is up, left as it is"
      : turn === 0
        ? "already right, skipped"
        : `will be turned ${describeTurn(turn)}`;
  const Badge = unsure ? CircleHelp : turn === 0 ? CheckIcon : RotateCw;
  const badgeColour = unsure ? "text-caution" : turn === 0 ? "text-tertiary" : "text-link";
  const description = `${file.name}: ${status}. Click to turn right, Shift-click to turn left.`;

  return (
    <button
      type="button"
      onClick={(e) => onTurn(e.shiftKey ? -1 : 1)}
      onKeyDown={(e) => {
        if (e.key === "ArrowRight" || e.key === "ArrowLeft") {
          e.preventDefault();
          onTurn(e.key === "ArrowRight" ? 1 : -1);
        }
      }}
      aria-label={description}
      title={description}
      style={style}
      className={`relative shrink-0 overflow-hidden rounded-control bg-surface ring-[0.5px] ring-separator ${className}`}
    >
      {url ? (
        <motion.img
          src={url}
          alt=""
          draggable={false}
          className={`size-full object-contain ${!check.checking && !unsure && turn === 0 ? "opacity-60" : ""}`}
          initial={false}
          animate={{ rotate: angle }}
          transition={TURN}
        />
      ) : (
        <div className="skeleton size-full" />
      )}
      {!check.checking && (
        <span className="absolute top-1 right-1 flex size-[18px] items-center justify-center rounded-pill bg-glass shadow-control">
          <Badge size={12} strokeWidth={2} className={badgeColour} />
        </span>
      )}
    </button>
  );
}
