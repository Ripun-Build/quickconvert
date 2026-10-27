import { test } from "node:test";
import assert from "node:assert/strict";
import { describeCount, describeFonts, describeMadeWith, describeOutput, describeRemoved, formatBytes, presetSummary, truncateMiddle } from "./format.ts";

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

test("remove-info results say what was taken out", () => {
  const all = { location: true, camera: true, date: true, other: true };
  const none = { location: false, camera: false, date: false, other: false };
  const cameraOnly = { ...none, camera: true };
  assert.equal(describeRemoved([{}]), undefined, "other actions");
  assert.equal(describeRemoved([{ removed: all }]), "Removed location, camera and date info and other details.");
  assert.equal(describeRemoved([{ removed: cameraOnly }]), "Removed camera info.");
  assert.equal(describeRemoved([{ removed: { ...none, other: true } }]), "Removed hidden info.");
  assert.equal(describeRemoved([{ removed: none }]), "No location or camera info found. Saved a clean copy anyway.");
  assert.equal(describeRemoved([{ removed: none }, { removed: none }]), "No location or camera info found in these images.");
  assert.equal(
    describeRemoved([{ removed: cameraOnly }, { removed: none }, { removed: { ...cameraOnly, location: true } }]),
    "Removed location and camera info (2 of 3 images had some).",
  );
  assert.equal(describeRemoved([{ removed: all }, { removed: all }]), "Removed location, camera and date info and other details from all 2 images.");
});

test("Convert to PDF results say how they were made", () => {
  assert.equal(describeMadeWith([{}]), undefined);
  assert.equal(describeMadeWith([{ madeWith: "keynote" }, { madeWith: "keynote" }]), "Made with Keynote.");
  assert.equal(
    describeMadeWith([{ madeWith: "pages" }, { madeWith: "quick" }]),
    "Made with Pages and Quick (built in). Quick copies are close to the original, not 100% the same.",
  );
});

test("font swaps are listed once, the rest counted", () => {
  assert.equal(describeFonts([{}, { fontsReplaced: [] }]), undefined);
  const swap = (from: string, to: string) => ({ from, to });
  assert.equal(
    describeFonts([{ fontsReplaced: [swap("Univers", "Helvetica Neue")] }, { fontsReplaced: [swap("Univers", "Helvetica Neue")] }]),
    "Fonts not on this Mac were replaced with similar ones: Univers → Helvetica Neue.",
  );
  const many = ["A", "B", "C", "D", "E", "F"].map((f) => swap(f, "Helvetica Neue"));
  assert.match(describeFonts([{ fontsReplaced: many }]) ?? "", /D → Helvetica Neue and 2 more\.$/);
});

test("the header names what's selected", () => {
  const f = (kind: "image" | "pdf" | "document") => ({ path: "/p/x", name: "x", kind, size: 1 });
  assert.equal(describeCount([f("image"), f("image")]), "2 images");
  assert.equal(describeCount([f("pdf")]), "1 PDF");
  assert.equal(describeCount([f("document"), f("document")]), "2 documents");
  assert.equal(describeCount([f("document")]), "1 document");
  assert.equal(describeCount([f("pdf"), f("document")]), "2 files");
});
