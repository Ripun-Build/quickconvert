import { test } from "node:test";
import assert from "node:assert/strict";
import { CHECKING, nearestAngle, plan, planTurns, summary, turnFor, type Check } from "./rotate.ts";
import type { FileInfo } from "./types.ts";

const file = (name: string): FileInfo => ({ path: `/p/${name}`, name, kind: "image", size: 1 });
const files = [file("straight.jpg"), file("sideways.jpg"), file("upside.jpg"), file("beach.jpg")];
const known = (upright: 0 | 90 | 180 | 270, extra = 0): Check => ({ checking: false, upright, extra });
const unknown = (extra = 0): Check => ({ checking: false, upright: null, extra });

const checks: Record<string, Check> = {
  "/p/straight.jpg": known(0),
  "/p/sideways.jpg": known(270),
  "/p/upside.jpg": known(180),
  "/p/beach.jpg": unknown(),
};

test("Straight turns each image by what it needs and skips the ones already straight", () => {
  const p = plan(files, checks, 0);
  assert.deepEqual(p.request, { action: "image_rotate", turns: { "/p/sideways.jpg": 270, "/p/upside.jpg": 180 } });
  assert.deepEqual(
    p.files.map((f) => f.name),
    ["sideways.jpg", "upside.jpg"],
  );
  assert.equal(p.unchanged, 1);
  assert.equal(p.unknown, 1, "beach.jpg: no text or faces, left as it is until clicked");
  assert.equal(summary(p, 4), "2 will be turned · 1 already right, skipped · 1 couldn't tell (click to turn)");
});

test("Left/Right/Upside down are relative to straight, so a mixed batch ends up all the same way", () => {
  const p = plan(files, checks, 90);
  assert.deepEqual(p.request, {
    action: "image_rotate",
    turns: { "/p/straight.jpg": 90, "/p/upside.jpg": 270, "/p/beach.jpg": 90 },
  });
  assert.equal(p.unchanged, 1, "sideways.jpg needs 270 to be straight, +90 = already right");
});

test("clicks add quarter turns on top of the detected one", () => {
  assert.equal(turnFor(unknown(1), 0), 90);
  assert.equal(turnFor(unknown(-1), 0), 270);
  assert.equal(turnFor(known(270, 1), 0), 0);
  assert.equal(turnFor(known(90, 5), 180), 0);
  const p = plan([file("beach.jpg")], { "/p/beach.jpg": unknown(2) }, 0);
  assert.equal(p.unknown, 0, "once turned by hand it's no longer 'couldn't tell'");
  assert.deepEqual(p.request, { action: "image_rotate", turns: { "/p/beach.jpg": 180 } });
});

test("while checking, nothing is counted as done", () => {
  const p = plan(files, { "/p/straight.jpg": known(0) }, 0);
  assert.equal(p.checking, 3);
  assert.equal(summary(p, 4), "Checking which way is up… 1 of 4");
  assert.equal(turnFor(CHECKING, 90), 90);
});

test("on-screen angle always moves the short way", () => {
  assert.equal(nearestAngle(0, 270), -90);
  assert.equal(nearestAngle(-90, 0), 0);
  assert.equal(nearestAngle(270, 0), 360);
  assert.equal(nearestAngle(90, 180), 180);
  assert.equal(nearestAngle(450, 180), 540);
});

test("pages use the same plan, keyed by page number", () => {
  const p = planTurns(["1", "2", "3"], { "1": known(90), "2": known(0), "3": unknown(1) }, 0);
  assert.deepEqual(p.turns, { "1": 90, "3": 90 });
  assert.deepEqual(p.keys, ["1", "3"]);
  assert.equal(summary(p, 3), "2 will be turned · 1 already right, skipped");
});
