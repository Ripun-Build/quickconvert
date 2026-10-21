import { test } from "node:test";
import assert from "node:assert/strict";
import { cropRequest, dragHandle, fitShape, moveBox, passportHint, roundBox, setBoxSide, turnBy, turnedSize } from "./crop.ts";

const photo = { width: 4000, height: 3000 };
const passport = 35 / 45;

test("quarter turns swap the sides and wrap around", () => {
  assert.deepEqual(turnedSize(photo, 90), { width: 3000, height: 4000 });
  assert.deepEqual(turnedSize(photo, 180), photo);
  assert.equal(turnBy(0, -1), 270);
  assert.equal(turnBy(270, 1), 0);
  assert.equal(turnBy(90, 1), 180);
});

test("a shape fits the largest centred box; free is the whole image", () => {
  assert.deepEqual(fitShape(photo, null), { x: 0, y: 0, width: 4000, height: 3000 });
  assert.deepEqual(fitShape(photo, 1), { x: 500, y: 0, width: 3000, height: 3000 });
  assert.deepEqual(fitShape(photo, 16 / 9), { x: 0, y: 375, width: 4000, height: 2250 });
  const p = fitShape(photo, passport);
  assert.equal(p.height, 3000);
  assert.ok(Math.abs(p.width / p.height - passport) < 1e-9);
});

test("moving stops at the image's edges", () => {
  const box = { x: 100, y: 100, width: 1000, height: 1000 };
  assert.deepEqual(moveBox(box, -500, 50, photo), { x: 0, y: 150, width: 1000, height: 1000 });
  assert.deepEqual(moveBox(box, 9999, 9999, photo), { x: 3000, y: 2000, width: 1000, height: 1000 });
});

test("free handles move only their own edges, inside the image, never below the minimum", () => {
  const box = { x: 1000, y: 1000, width: 1000, height: 1000 };
  assert.deepEqual(dragHandle("se", box, { x: 2500, y: 2200 }, photo, null), { x: 1000, y: 1000, width: 1500, height: 1200 });
  assert.deepEqual(dragHandle("w", box, { x: -50, y: 0 }, photo, null), { x: 0, y: 1000, width: 2000, height: 1000 });
  assert.deepEqual(dragHandle("n", box, { x: 0, y: 5000 }, photo, null), { x: 1000, y: 1984, width: 1000, height: 16 });
});

test("with a shape, a corner follows the further axis and keeps the shape", () => {
  const box = { x: 1000, y: 500, width: 1000, height: 1000 };
  // Pointer moved further along y: the square grows to 1500 from the top-left anchor.
  assert.deepEqual(dragHandle("se", box, { x: 2100, y: 2000 }, photo, 1), { x: 1000, y: 500, width: 1500, height: 1500 });
  // Can't grow past the bottom edge: 2500 px of room below the anchor.
  assert.deepEqual(dragHandle("se", box, { x: 9000, y: 9000 }, photo, 1), { x: 1000, y: 500, width: 2500, height: 2500 });
  // Dragging past the anchor just shrinks to the minimum.
  assert.deepEqual(dragHandle("nw", box, { x: 3000, y: 3000 }, photo, 1), { x: 1984, y: 1484, width: 16, height: 16 });
});

test("with a shape, a side handle grows around the centre line and slides to stay inside", () => {
  const box = { x: 0, y: 0, width: 1000, height: 1000 };
  const out = dragHandle("e", box, { x: 2000, y: 0 }, photo, 1);
  assert.deepEqual(out, { x: 0, y: 0, width: 2000, height: 2000 });
});

test("typing a size keeps the centre and the shape", () => {
  const box = { x: 1000, y: 1000, width: 1000, height: 1000 };
  assert.deepEqual(setBoxSide("width", 2000, box, photo, 1), { x: 500, y: 500, width: 2000, height: 2000 });
  assert.deepEqual(setBoxSide("height", 500, box, photo, null), { x: 1000, y: 1250, width: 1000, height: 500 });
  // Too big for the image: capped, and pushed back inside.
  assert.deepEqual(setBoxSide("width", 9999, box, photo, 1), { x: 0, y: 0, width: 3000, height: 3000 });
  assert.equal(setBoxSide("width", 0, box, photo, 1), box);
});

test("the request uses whole pixels inside the image and carries the turn", () => {
  const image = turnedSize(photo, 90);
  const box = { x: 10.4, y: 20.6, width: 3000.2, height: 99.5 };
  assert.deepEqual(roundBox(box, image), { x: 10, y: 21, width: 2990, height: 100 });
  assert.deepEqual(cropRequest(box, image, 90), {
    action: "image_crop",
    rect: { x: 10, y: 21, width: 2990, height: 100 },
    turn: 90,
  });
});

test("passport hint says whether the crop prints sharply", () => {
  assert.match(passportHint({ x: 0, y: 0, width: 700, height: 900 }), /^Sharp enough/);
  assert.match(passportHint({ x: 0, y: 0, width: 350, height: 450 }), /small/);
});
