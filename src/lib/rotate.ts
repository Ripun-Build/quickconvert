// Rotate view logic: every image ends up the way the user picked (Straight by default), using the
// turn Rust detected for each one plus any turns the user added by clicking. Pure, so it runs
// under `node --test`.
import type { FileInfo, JobRequest, Turn } from "./types";

/** How every image should end up, as a turn from straight. */
export const TARGETS: { value: string; label: string }[] = [
  { value: "0", label: "Straight" },
  { value: "270", label: "Left" },
  { value: "90", label: "Right" },
  { value: "180", label: "Upside down" },
];

export interface Check {
  /** Still asking Rust. */
  checking: boolean;
  /** Clockwise degrees that make the image straight; null = couldn't tell. */
  upright: Turn | null;
  /** Quarter turns added by the user (click = +1, Shift-click / ← = −1). */
  extra: number;
}

export const CHECKING: Check = { checking: true, upright: null, extra: 0 };

const normalise = (degrees: number) => ((((degrees % 360) + 360) % 360) as Turn);

/** The turn to apply: make it straight (if known), then turn to the target, then the user's extra turns. */
export function turnFor(check: Check, target: Turn): Turn {
  return normalise((check.upright ?? 0) + target + check.extra * 90);
}

/** Plain words for a turn. */
export function describeTurn(turn: Turn): string {
  return turn === 90 ? "a quarter turn right" : turn === 180 ? "upside down" : turn === 270 ? "a quarter turn left" : "no turn";
}

export interface Plan {
  /** Files that need turning, in selection order. */
  files: FileInfo[];
  request: JobRequest;
  /** Already the way the user wants: skipped, not saved again. */
  unchanged: number;
  /** Couldn't tell which way is up and the user hasn't turned it. */
  unknown: number;
  checking: number;
}

export function plan(files: FileInfo[], checks: Record<string, Check>, target: Turn): Plan {
  const turns: Record<string, Turn> = {};
  const toTurn: FileInfo[] = [];
  let unchanged = 0;
  let unknown = 0;
  let checking = 0;
  for (const file of files) {
    const check = checks[file.path] ?? CHECKING;
    const unsure = !check.checking && check.upright === null && check.extra === 0;
    if (check.checking) checking++;
    else if (unsure) unknown++;
    const turn = turnFor(check, target);
    if (turn === 0) {
      if (!check.checking && !unsure) unchanged++;
      continue;
    }
    turns[file.path] = turn;
    toTurn.push(file);
  }
  return { files: toTurn, request: { action: "image_rotate", turns }, unchanged, unknown, checking };
}

/** One line under the grid, e.g. "3 will be turned · 1 already right · 1 couldn't tell (click it to turn)". */
export function summary(p: Plan, total: number): string {
  if (p.checking > 0) return `Checking which way is up… ${total - p.checking} of ${total}`;
  const parts = [];
  if (p.files.length > 0) parts.push(`${p.files.length} will be turned`);
  if (p.unchanged > 0) parts.push(`${p.unchanged} already right, skipped`);
  if (p.unknown > 0) parts.push(`${p.unknown} couldn't tell (click to turn)`);
  return parts.join(" · ");
}

/** Next on-screen angle for `turn`, moving the short way from `previous` (no spinning backwards). */
export function nearestAngle(previous: number, turn: Turn): number {
  const delta = ((((turn - previous) % 360) + 540) % 360) - 180;
  return previous + delta;
}
