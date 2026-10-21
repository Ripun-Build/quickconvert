import { test } from "node:test";
import assert from "node:assert/strict";
import { allPages, clickPage, formatPages, parsePages, pixels } from "./pages.ts";

test("pages format as compact ranges", () => {
  assert.equal(formatPages([5, 1, 2, 3, 3]), "1-3, 5");
  assert.equal(formatPages([7]), "7");
  assert.equal(formatPages([]), "");
  assert.equal(formatPages(allPages(4)), "1-4");
});

test("typed ranges parse like the Rust side", () => {
  assert.deepEqual(parsePages("1-3, 5", 10), { pages: [1, 2, 3, 5] });
  assert.deepEqual(parsePages(" 5, 2-3,", 10), { pages: [2, 3, 5] });
  assert.deepEqual(parsePages("8-12", 10), { error: "This PDF has only 10 pages." });
  assert.deepEqual(parsePages("3-1", 10), { error: "Write pages like 1-3, 5." });
  assert.deepEqual(parsePages("x", 10), { error: "Write pages like 1-3, 5." });
  assert.deepEqual(parsePages("", 10), { error: "Pick at least one page." });
});

test("clicking toggles; shift-click fills the range with the anchor's state", () => {
  let sel = clickPage(new Set(), 2, null, false);
  assert.deepEqual([...sel], [2]);
  sel = clickPage(sel, 5, 2, true);
  assert.deepEqual([...sel].sort(), [2, 3, 4, 5]);
  sel = clickPage(sel, 3, null, false);
  assert.deepEqual([...sel].sort(), [2, 4, 5]);
  // Anchor 3 is now off, so shift-clicking 5 turns 3–5 off.
  sel = clickPage(sel, 5, 3, true);
  assert.deepEqual([...sel], [2]);
});

test("pixel size at a resolution", () => {
  assert.equal(pixels(595.28, 150), 1240); // A4 width at 150 DPI
  assert.equal(pixels(841.89, 300), 3508);
  assert.equal(pixels(0, 72), 1);
});
