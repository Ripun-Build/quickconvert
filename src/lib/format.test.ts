import { test } from "node:test";
import assert from "node:assert/strict";
import { describeOutput, formatBytes, presetSummary, truncateMiddle } from "./format.ts";

test("sizes use decimal units like Finder", () => {
  assert.equal(formatBytes(512), "512 B");
  assert.equal(formatBytes(48_210), "48 KB");
  assert.equal(formatBytes(2_400_000), "2.4 MB");
});

test("output details read like the spec", () => {
  assert.equal(describeOutput({ bytes: 48_210, width: 200, height: 230, quality: 78 }), "48 KB · 200×230 · quality 78");
  assert.equal(describeOutput({ bytes: 1_200_000 }), "1.2 MB");
  assert.equal(describeOutput({ bytes: 90_000, width: 64, height: 64 }), "90 KB · 64×64");
});

test("preset summaries", () => {
  assert.equal(
    presetSummary({ format: "jpg", width: 200, height: 230, minKB: 20, maxKB: 50, dpi: 200 }),
    "200 × 230 px · 20–50 KB · JPG · 200 DPI",
  );
  assert.equal(presetSummary({ format: "jpg", longEdge: 1600, maxKB: 500 }), "1600 px long edge · under 500 KB · JPG");
  assert.equal(presetSummary({ format: "jpg", maxKB: 1000 }), "under 1 MB · JPG");
  assert.equal(presetSummary({ format: "jpg", aspect: "35:45", minKB: 20 }), "35:45 crop · at least 20 KB · JPG");
});

test("long names keep their start and extension", () => {
  assert.equal(truncateMiddle("IMG_20240412_093011_0412.jpg", 20), "IMG_202404…_0412.jpg");
  assert.equal(truncateMiddle("short.jpg"), "short.jpg");
});
