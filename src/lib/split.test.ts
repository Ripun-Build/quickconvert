import { test } from "node:test";
import assert from "node:assert/strict";
import { describePart, everyN, partOf, parts, toggleStart } from "./split.ts";

test("clicking pages starts and stops parts; page 1 always starts part 1", () => {
  let starts = toggleStart(new Set(), 4);
  starts = toggleStart(starts, 7);
  assert.deepEqual(parts(9, starts), [[1, 2, 3], [4, 5, 6], [7, 8, 9]]);
  starts = toggleStart(starts, 4);
  assert.deepEqual(parts(9, starts), [[1, 2, 3, 4, 5, 6], [7, 8, 9]]);
  assert.deepEqual(toggleStart(starts, 1), starts, "page 1 can't be toggled");
});

test("quick choices", () => {
  assert.deepEqual(parts(3, everyN(3, 1)), [[1], [2], [3]]);
  assert.deepEqual(parts(5, everyN(5, 2)), [[1, 2], [3, 4], [5]]);
  assert.deepEqual(parts(4, new Set()), [[1, 2, 3, 4]]);
});

test("each page knows its part, and parts read naturally", () => {
  assert.deepEqual(partOf(6, new Set([3, 5])), [1, 1, 2, 2, 3, 3]);
  assert.equal(describePart([4, 5, 6, 7]), "pages 4–7");
  assert.equal(describePart([3]), "page 3");
});
