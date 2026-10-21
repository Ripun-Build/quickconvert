// Resize editor maths. Pure module so it runs under `node --test`.
import type { JobRequest } from "./types";

export interface Size {
  width: number;
  height: number;
}

export type Handle = "corner" | "right" | "bottom";

const clamp = (v: number, min: number, max: number) => Math.min(max, Math.max(min, v));

/** Never larger than the original (no upscaling), never below 1 px. */
export function clampSize(size: Size, original: Size): Size {
  return {
    width: clamp(Math.round(size.width), 1, original.width),
    height: clamp(Math.round(size.height), 1, original.height),
  };
}

/** Scale the original by `ratio` (0–1], keeping its aspect. */
export function scaled(original: Size, ratio: number): Size {
  const r = clamp(ratio, 0, 1);
  return clampSize({ width: original.width * r, height: original.height * r }, original);
}

/**
 * New size while dragging `handle` to `point` (image pixels, measured from the image's top-left).
 * With the aspect locked every handle scales proportionally; the corner follows whichever axis
 * the pointer has moved further along.
 */
export function dragTo(handle: Handle, point: { x: number; y: number }, current: Size, original: Size, locked: boolean): Size {
  const rx = point.x / original.width;
  const ry = point.y / original.height;
  if (locked) {
    const ratio = handle === "right" ? rx : handle === "bottom" ? ry : Math.max(rx, ry);
    return scaled(original, ratio);
  }
  return clampSize(
    {
      width: handle === "bottom" ? current.width : point.x,
      height: handle === "right" ? current.height : point.y,
    },
    original,
  );
}

/** Typing a width or height; the other side follows when the aspect is locked. */
export function setSide(side: "width" | "height", value: number, current: Size, original: Size, locked: boolean): Size {
  if (!Number.isFinite(value) || value <= 0) return current;
  if (!locked) return clampSize({ ...current, [side]: value }, original);
  const ratio = side === "width" ? value / original.width : value / original.height;
  return scaled(original, ratio);
}

/** Default: long edge 1600 px (SPECS §6; Settings can change it), or the original if smaller. */
export function defaultSize(original: Size, longEdge = 1600): Size {
  return scaled(original, Math.min(1, longEdge / Math.max(original.width, original.height)));
}

export const percentOf = (size: Size, original: Size) => Math.round((size.width / original.width) * 1000) / 10;

/**
 * Request for the job. One file: exactly this size. Several files (the editor previews the first):
 * locked → the same percentage for each, so every image keeps its own shape; unlocked → this exact size.
 */
export function resizeRequest(size: Size, original: Size, locked: boolean, fileCount: number): JobRequest {
  if (fileCount > 1 && locked) {
    return { action: "image_resize", mode: { kind: "percent", percent: (size.width / original.width) * 100 } };
  }
  return {
    action: "image_resize",
    mode: { kind: "exact", width: size.width, height: size.height, keepAspect: false },
  };
}
