// Run with `npm test` (Node's built-in runner; types are stripped at runtime).
import { test } from "node:test";
import assert from "node:assert/strict";
import { ACTION_SPECS, ACTIONS, PAGE_RANGE, buildMenu, filterMenu, flatActions, withLabel, type InputKind } from "./actions.ts";

const ids = (kinds: InputKind[]) => flatActions(buildMenu(kinds)).map((a) => a.id);
const suggested = (kinds: InputKind[]) =>
  buildMenu(kinds)
    .sections.find((s) => s.title === "Suggested")
    ?.items.map((i) => i.action.id);
/** Every listed action → its reason ("" when enabled). */
const listed = (kinds: InputKind[]) =>
  Object.fromEntries(buildMenu(kinds).sections.flatMap((s) => s.items).map((i) => [i.action.id, i.reason ?? ""]));

test("one image suggests convert, compress, resize, combine", () => {
  assert.deepEqual(suggested(["image"]), ["image_convert", "image_compress", "image_resize", "images_to_pdf"]);
});

test("many images suggest combine into PDF first", () => {
  assert.deepEqual(suggested(["image", "image", "image", "image", "image"]), [
    "images_to_pdf",
    "image_compress",
    "image_resize",
  ]);
  assert.equal(ids(["image", "image"])[0], "images_to_pdf");
});

test("every action is listed; the ones that don't fit are disabled with a reason", () => {
  const all = listed(["image", "image"]);
  assert.equal(Object.keys(all).length, ACTIONS.length);
  assert.equal(all.images_to_pdf, "");
  assert.equal(all.pdf_to_images, "Needs one PDF");
  assert.equal(all.pdf_merge, "Needs 2+ PDFs");
  assert.equal(all.image_crop, "Works on one image at a time");
  // Runnable actions (and their 1–9 numbers) skip the disabled ones.
  assert.deepEqual(ids(["image", "image"]), [
    "images_to_pdf",
    "image_compress",
    "image_resize",
    "image_convert",
    "image_rotate",
    "strip_metadata",
    "apply_preset",
  ]);
});

test("enabled actions come first within a group", () => {
  const convert = buildMenu(["image"]).sections.find((s) => s.title === "Convert")!;
  assert.deepEqual(
    convert.items.map((i) => [i.action.id, i.enabled]),
    [
      ["pdf_to_images", false],
      ["pdf_merge", false],
      ["office_to_pdf", false],
    ],
  );
  const pdfConvert = buildMenu(["pdf"]).sections.find((s) => s.title === "Convert")!;
  assert.deepEqual(
    pdfConvert.items.map((i) => [i.action.id, i.enabled]),
    [
      ["images_to_pdf", false],
      ["image_convert", false],
      ["pdf_merge", false],
      ["office_to_pdf", false],
    ],
  );
});

test("one PDF suggests PDF to images; image actions say they need images", () => {
  assert.deepEqual(suggested(["pdf"]), ["pdf_to_images", "pdf_compress", "pdf_split"]);
  const all = listed(["pdf"]);
  assert.equal(all.image_convert, "Needs images");
  assert.equal(all.pdf_merge, "Needs 2+ PDFs");
  assert.deepEqual(ids(["pdf"]), ["pdf_to_images", "pdf_compress", "pdf_split", "pdf_rotate"]);
});

test("two PDFs suggest merge, and PDF to images works on one at a time", () => {
  assert.deepEqual(suggested(["pdf", "pdf"]), ["pdf_merge", "pdf_compress"]);
  assert.equal(listed(["pdf", "pdf"]).pdf_to_images, "Works on one PDF at a time");
  assert.equal(listed(["pdf", "pdf"]).pdf_split, "Works on one PDF at a time");
  assert.equal(listed(["pdf", "pdf"]).pdf_rotate, "Works on one PDF at a time");
});

test("mixed images and PDFs are incompatible with per-type groups", () => {
  const menu = buildMenu(["image", "pdf", "image"]);
  assert.equal(menu.incompatible, true);
  assert.deepEqual(
    menu.sections.map((s) => [s.title, s.scope]),
    [
      ["For the 2 images", "image"],
      ["For the 1 PDF", "pdf"],
      ["Not available", undefined],
    ],
  );
  assert.deepEqual(
    menu.sections[1].items.map((i) => i.action.id),
    ["pdf_to_images", "pdf_split", "pdf_rotate", "pdf_compress"],
  );
  assert.deepEqual(
    menu.sections[2].items.map((i) => [i.action.id, i.reason]),
    [
      ["pdf_merge", "Needs 2+ PDFs"],
      ["image_crop", "Works on one image at a time"],
      ["office_to_pdf", "Needs Word, PowerPoint or Excel files"],
    ],
  );
});

test("empty selection has no actions", () => {
  assert.deepEqual(buildMenu([]), { sections: [], incompatible: false });
});

test("search matches label words, keyword prefixes, and small typos", () => {
  const menu = buildMenu(["image"]);
  const search = (q: string) => flatActions(filterMenu(menu, q)).map((a) => a.id);
  assert.deepEqual(search("comp"), ["image_compress"]);
  assert.deepEqual(search("Comp"), ["image_compress"]);
  assert.deepEqual(search("passport"), ["image_crop", "apply_preset"]);
  assert.deepEqual(search("sideways"), ["image_rotate"]);
  assert.deepEqual(search("gps"), ["strip_metadata"]);
  assert.deepEqual(search("heic"), ["image_convert"]);
  assert.deepEqual(search("into pdf"), ["images_to_pdf"]);
  assert.deepEqual(search("cmprs"), ["image_compress"]);
  assert.deepEqual(search("webp"), ["image_convert"]);
  assert.deepEqual(search("  "), flatActions(menu).map((a) => a.id));
  assert.deepEqual(search("zzz"), []);
});

test("every action has a unique id", () => {
  assert.equal(new Set(ACTIONS.map((a) => a.id)).size, ACTIONS.length);
});

test("default options build the requests Rust expects", () => {
  const request = (id: keyof typeof ACTION_SPECS, overrides = {}) => {
    const spec = ACTION_SPECS[id]!;
    return spec.toRequest({ ...spec.defaults, ...overrides });
  };
  assert.deepEqual(request("images_to_pdf"), { action: "images_to_pdf", pageSize: "fit", margin: "none" });
  assert.deepEqual(request("image_convert"), { action: "image_convert", format: "jpg", quality: 85 });
  assert.deepEqual(request("image_resize"), { action: "image_resize", mode: { kind: "long_edge", px: 1600 } });
  assert.deepEqual(request("image_resize", { mode: "exact", width: "800" }), {
    action: "image_resize",
    mode: { kind: "exact", width: 800, height: 1600, keepAspect: true },
  });
  assert.deepEqual(request("pdf_merge"), { action: "pdf_merge" });
  assert.deepEqual(request("pdf_to_images"), { action: "pdf_to_images", format: "jpg", dpi: 150, pages: { kind: "all" } });
  assert.deepEqual(request("pdf_to_images", { pages: "range", range: "1-3,5", format: "png", dpi: "300" }), {
    action: "pdf_to_images",
    format: "png",
    dpi: 300,
    pages: { kind: "range", spec: "1-3,5" },
  });
});

test("quality is hidden for PNG; resize shows only the active mode's fields", () => {
  const convert = ACTION_SPECS.image_convert!;
  const quality = convert.fields.find((f) => f.key === "quality")!;
  assert.equal(quality.visible?.({ format: "png" }), false);
  assert.equal(quality.visible?.({ format: "webp" }), true);
  const resize = ACTION_SPECS.image_resize!;
  const shown = resize.fields.filter((f) => !f.visible || f.visible({ mode: "exact" })).map((f) => f.key);
  assert.deepEqual(shown, ["mode", "width", "height", "keepAspect"]);
});

test("page range field accepts lists and ranges only", () => {
  for (const ok of ["1", "1-3", "1-3,5", " 2 , 4 - 6 ", "1,"]) assert.ok(PAGE_RANGE.test(ok), ok);
  for (const bad of ["", "a", "1--3", "-3", "1-3-5", "1.5"]) assert.ok(!PAGE_RANGE.test(bad) || !/\d/.test(bad), bad);
});

test("Word, PowerPoint and Excel files suggest Convert to PDF; mixed with photos they get their own group", () => {
  assert.deepEqual(suggested(["document", "document"]), ["office_to_pdf"]);
  assert.equal(listed(["document"]).image_convert, "Needs images");
  const mixed = buildMenu(["image", "document", "document"]);
  assert.equal(mixed.incompatible, true);
  assert.deepEqual(
    mixed.sections.map((s) => [s.title, s.scope]).slice(0, 2),
    [
      ["For the 1 image", "image"],
      ["For the 2 documents", "document"],
    ],
  );
  assert.deepEqual(
    mixed.sections[1].items.map((i) => i.action.id),
    ["office_to_pdf"],
  );
});

test("Convert to PDF can be shown under the selection's name", () => {
  const menu = withLabel(buildMenu(["document"]), "office_to_pdf", "Word to PDF");
  assert.equal(menu.sections[0].items[0].action.label, "Word to PDF");
  assert.equal(ACTIONS.find((a) => a.id === "office_to_pdf")!.label, "Convert to PDF", "registry unchanged");
});
