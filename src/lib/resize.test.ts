import { test } from "node:test";
import assert from "node:assert/strict";
import { defaultSize, dragTo, percentOf, resizeRequest, scaled, setSide } from "./resize.ts";

const photo = { width: 4032, height: 3024 };

test("default is long edge 1600, never larger than the original", () => {
  assert.deepEqual(defaultSize(photo), { width: 1600, height: 1200 });
  assert.deepEqual(defaultSize({ width: 800, height: 600 }), { width: 800, height: 600 });
});

test("locked corner drag keeps the aspect and follows the further axis", () => {
  const now = scaled(photo, 0.5);
  assert.deepEqual(dragTo("corner", { x: 2016, y: 100 }, now, photo, true), { width: 2016, height: 1512 });
  assert.deepEqual(dragTo("corner", { x: 100, y: 1512 }, now, photo, true), { width: 2016, height: 1512 });
});

test("unlocked edge drags change one side only", () => {
  const now = { width: 2000, height: 1500 };
  assert.deepEqual(dragTo("right", { x: 1000, y: 9999 }, now, photo, false), { width: 1000, height: 1500 });
  assert.deepEqual(dragTo("bottom", { x: 9999, y: 700 }, now, photo, false), { width: 2000, height: 700 });
  assert.deepEqual(dragTo("corner", { x: 1234, y: 567 }, now, photo, false), { width: 1234, height: 567 });
});

test("drags clamp to 1 px and the original size (no upscaling)", () => {
  const now = { width: 2000, height: 1500 };
  assert.deepEqual(dragTo("corner", { x: 99999, y: 99999 }, now, photo, false), photo);
  assert.deepEqual(dragTo("corner", { x: -50, y: -50 }, now, photo, false), { width: 1, height: 1 });
  assert.deepEqual(dragTo("corner", { x: 99999, y: 0 }, now, photo, true), photo);
});

test("typing a side updates the other when locked", () => {
  const now = defaultSize(photo);
  assert.deepEqual(setSide("width", 1008, now, photo, true), { width: 1008, height: 756 });
  assert.deepEqual(setSide("height", 1512, now, photo, true), { width: 2016, height: 1512 });
  assert.deepEqual(setSide("width", 900, now, photo, false), { width: 900, height: 1200 });
  assert.deepEqual(setSide("width", 50000, now, photo, false), { width: 4032, height: 1200 });
  assert.deepEqual(setSide("width", Number.NaN, now, photo, true), now);
});

test("percent display and job requests", () => {
  const half = scaled(photo, 0.5);
  assert.equal(percentOf(half, photo), 50);
  assert.deepEqual(resizeRequest(half, photo, true, 1), {
    action: "image_resize",
    mode: { kind: "exact", width: 2016, height: 1512, keepAspect: false },
  });
  assert.deepEqual(resizeRequest(half, photo, true, 5), {
    action: "image_resize",
    mode: { kind: "percent", percent: 50 },
  });
  assert.deepEqual(resizeRequest({ width: 800, height: 800 }, photo, false, 5).mode, {
    kind: "exact",
    width: 800,
    height: 800,
    keepAspect: false,
  });
});
