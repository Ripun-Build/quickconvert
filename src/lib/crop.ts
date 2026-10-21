// Crop editor maths. Pure module so it runs under `node --test`.
import type { JobRequest, Turn } from "./types";

export interface Size {
  width: number;
  height: number;
}

/** Crop area in image pixels (after any turn). Fractional while dragging; rounded for the job. */
export interface Box {
  x: number;
  y: number;
  width: number;
  height: number;
}

export type CropHandle = "n" | "s" | "e" | "w" | "ne" | "nw" | "se" | "sw";

export interface Shape {
  id: string;
  label: string;
  /** Width ÷ height; null = free. */
  ratio: number | null;
}

export const SHAPES: readonly Shape[] = [
  { id: "free", label: "Free", ratio: null },
  { id: "square", label: "Square", ratio: 1 },
  { id: "4:3", label: "4:3", ratio: 4 / 3 },
  { id: "3:4", label: "3:4", ratio: 3 / 4 },
  { id: "16:9", label: "16:9", ratio: 16 / 9 },
  { id: "passport", label: "Passport 35×45", ratio: 35 / 45 },
];

/** Smallest crop side in pixels (or the image's own side when that's smaller). */
export const MIN_SIDE = 16;

const clamp = (v: number, min: number, max: number) => Math.min(max, Math.max(min, v));
const minSide = (image: Size) => Math.min(MIN_SIDE, image.width, image.height);

/** Image size after `turn` (quarter turns swap the sides). */
export function turnedSize(original: Size, turn: Turn): Size {
  return turn === 90 || turn === 270 ? { width: original.height, height: original.width } : original;
}

/** One quarter turn clockwise (1) or anticlockwise (-1). */
export function turnBy(turn: Turn, step: 1 | -1): Turn {
  return ((((turn + step * 90) % 360) + 360) % 360) as Turn;
}

/** Largest box of `ratio` centred in the image; the whole image when free. */
export function fitShape(image: Size, ratio: number | null): Box {
  if (!ratio) return { x: 0, y: 0, width: image.width, height: image.height };
  const width = Math.min(image.width, image.height * ratio);
  const height = width / ratio;
  return { x: (image.width - width) / 2, y: (image.height - height) / 2, width, height };
}

/** Slide the box by (dx, dy), stopping at the image's edges. */
export function moveBox(box: Box, dx: number, dy: number, image: Size): Box {
  return {
    ...box,
    x: clamp(box.x + dx, 0, image.width - box.width),
    y: clamp(box.y + dy, 0, image.height - box.height),
  };
}

/**
 * Box while dragging `handle` to `point` (image pixels). The opposite corner or edge stays put and
 * the box never leaves the image. With a shape the box keeps it: a corner follows whichever axis the
 * pointer moved further along, a side handle grows the box around its centre line.
 */
export function dragHandle(handle: CropHandle, start: Box, point: { x: number; y: number }, image: Size, ratio: number | null): Box {
  const min = minSide(image);
  const west = handle.includes("w");
  const east = handle.includes("e");
  const north = handle.includes("n");
  const south = handle.includes("s");
  const right = start.x + start.width;
  const bottom = start.y + start.height;

  if (!ratio) {
    const x0 = west ? clamp(point.x, 0, right - min) : start.x;
    const x1 = east ? clamp(point.x, start.x + min, image.width) : right;
    const y0 = north ? clamp(point.y, 0, bottom - min) : start.y;
    const y1 = south ? clamp(point.y, start.y + min, image.height) : bottom;
    return { x: x0, y: y0, width: x1 - x0, height: y1 - y0 };
  }

  // The point that stays put, and how far the box may grow from it.
  const ax = west ? right : east ? start.x : start.x + start.width / 2;
  const ay = north ? bottom : south ? start.y : start.y + start.height / 2;
  const roomX = west ? ax : east ? image.width - ax : image.width;
  const roomY = north ? ay : south ? image.height - ay : image.height;
  const wantW = west ? ax - point.x : east ? point.x - ax : 0;
  const wantH = north ? ay - point.y : south ? point.y - ay : 0;
  const most = Math.min(roomX, roomY * ratio);
  const width = Math.min(Math.max(wantW, wantH * ratio, min, min * ratio), most);
  const height = width / ratio;
  return {
    x: west ? ax - width : east ? ax : clamp(ax - width / 2, 0, image.width - width),
    y: north ? ay - height : south ? ay : clamp(ay - height / 2, 0, image.height - height),
    width,
    height,
  };
}

/** Typing a width or height: the box keeps its centre (and shape), staying inside the image. */
export function setBoxSide(side: "width" | "height", value: number, box: Box, image: Size, ratio: number | null): Box {
  if (!Number.isFinite(value) || value <= 0) return box;
  const min = minSide(image);
  let width: number;
  let height: number;
  if (ratio) {
    const wanted = side === "width" ? value : value * ratio;
    width = Math.min(Math.max(wanted, min, min * ratio), image.width, image.height * ratio);
    height = width / ratio;
  } else {
    width = side === "width" ? clamp(value, min, image.width) : box.width;
    height = side === "height" ? clamp(value, min, image.height) : box.height;
  }
  const cx = box.x + box.width / 2;
  const cy = box.y + box.height / 2;
  return {
    x: clamp(cx - width / 2, 0, image.width - width),
    y: clamp(cy - height / 2, 0, image.height - height),
    width,
    height,
  };
}

/** Whole pixels, still inside the image. */
export function roundBox(box: Box, image: Size): Box {
  const x = clamp(Math.round(box.x), 0, image.width - 1);
  const y = clamp(Math.round(box.y), 0, image.height - 1);
  return {
    x,
    y,
    width: clamp(Math.round(box.width), 1, image.width - x),
    height: clamp(Math.round(box.height), 1, image.height - y),
  };
}

/** 35 × 45 mm at 300 DPI, the usual print quality for passport photos. */
const PASSPORT_PRINT = { width: 413, height: 531 };

/** Plain-language check for the Passport shape: is the crop big enough to print sharply? */
export function passportHint(box: Box): string {
  const sharp = Math.round(box.width) >= PASSPORT_PRINT.width && Math.round(box.height) >= PASSPORT_PRINT.height;
  return sharp ? "Sharp enough to print at 35 × 45 mm" : "A bit small to print sharply at 35 × 45 mm";
}

export function cropRequest(box: Box, image: Size, turn: Turn): JobRequest {
  return { action: "image_crop", rect: roundBox(box, image), turn };
}
