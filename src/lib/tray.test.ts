import { test } from "node:test";
import assert from "node:assert/strict";
import { fromFiles, moveTo, nudge, pageCount, pagePreview, rotateItem, sortByName, toggleOut, trayRequest } from "./tray.ts";
import type { FileInfo } from "./types.ts";

const file = (name: string): FileInfo => ({ path: `/p/${name}`, name, kind: "image", size: 1 });
const names = (items: { file: FileInfo }[]) => items.map((i) => i.file.name);
const items = fromFiles(["a.jpg", "b.jpg", "c.jpg", "d.jpg"].map(file));

test("moving drops the photo before the target, or at the end", () => {
  assert.deepEqual(names(moveTo(items, 0, 2)), ["b.jpg", "a.jpg", "c.jpg", "d.jpg"]);
  assert.deepEqual(names(moveTo(items, 3, 0)), ["d.jpg", "a.jpg", "b.jpg", "c.jpg"]);
  assert.deepEqual(names(moveTo(items, 1, 4)), ["a.jpg", "c.jpg", "d.jpg", "b.jpg"]);
  assert.deepEqual(names(moveTo(items, 2, 2)), names(items), "dropped on itself: no change");
});

test("keyboard nudges move one place and stop at the ends", () => {
  assert.deepEqual(names(nudge(items, 1, 1)), ["a.jpg", "c.jpg", "b.jpg", "d.jpg"]);
  assert.deepEqual(names(nudge(items, 1, -1)), ["b.jpg", "a.jpg", "c.jpg", "d.jpg"]);
  assert.deepEqual(names(nudge(items, 0, -1)), names(items));
  assert.deepEqual(names(nudge(items, 3, 1)), names(items));
});

test("leaving out and turning; only kept photos go to the job, in tray order", () => {
  let tray = toggleOut(items, 1);
  tray = rotateItem(tray, 2, 1);
  tray = rotateItem(tray, 3, -1);
  tray = moveTo(tray, 3, 0);
  assert.equal(pageCount(tray), 3);
  const { request, files } = trayRequest(tray, "a4", "small");
  assert.deepEqual(
    files.map((f) => f.name),
    ["d.jpg", "a.jpg", "c.jpg"],
  );
  assert.deepEqual(request, {
    action: "images_to_pdf",
    pageSize: "a4",
    margin: "small",
    turns: { "/p/c.jpg": 90, "/p/d.jpg": 270 },
  });
  assert.equal(pageCount(toggleOut(tray, 2)), 4, "click again to bring it back");
});

test("name order is natural", () => {
  const tray = fromFiles(["IMG_10.jpg", "img_2.jpg", "IMG_1.jpg"].map(file));
  assert.deepEqual(names(sortByName(tray)), ["IMG_1.jpg", "img_2.jpg", "IMG_10.jpg"]);
});

test("page previews follow the real layout", () => {
  const tall = { width: 300, height: 400 };
  const a4 = pagePreview(tall, 0, "a4", "none");
  assert.ok(Math.abs(a4.aspect - 595.276 / 841.89) < 1e-9);
  assert.ok(Math.abs(a4.box.w - 1) < 1e-9 || Math.abs(a4.box.h - 1) < 1e-9, "fills one side");
  // Turned sideways: a landscape A4 page.
  assert.ok(pagePreview(tall, 90, "a4", "none").aspect > 1);
  // Small margin keeps at least 10 mm all round.
  const small = pagePreview(tall, 0, "letter", "small");
  assert.ok(small.box.x * 612 >= (10 * 72) / 25.4 - 1e-6);
  assert.ok(small.box.y * 792 >= (10 * 72) / 25.4 - 1e-6);
  // Fit: the page is the photo's own shape.
  assert.equal(pagePreview(tall, 0, "fit", "none").aspect, 300 / 400);
  assert.deepEqual(pagePreview(tall, 0, "fit", "none").box, { x: 0, y: 0, w: 1, h: 1 });
  assert.equal(pagePreview(tall, 270, "fit", "none").aspect, 400 / 300);
});
