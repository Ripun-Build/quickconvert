// Run with `npm test` (Node's built-in runner; types are stripped at runtime).
import { test } from "node:test";
import assert from "node:assert/strict";
import { ACTIONS, buildMenu, filterMenu, flatActions, type InputKind } from "./actions.ts";

const ids = (kinds: InputKind[]) => flatActions(buildMenu(kinds)).map((a) => a.id);
const suggested = (kinds: InputKind[]) => buildMenu(kinds).sections.find((s) => s.title === "Suggested")?.actions.map((a) => a.id);

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

test("non-suggested valid actions follow, grouped", () => {
  const menu = buildMenu(["image", "image"]);
  assert.deepEqual(
    menu.sections.map((s) => s.title),
    ["Suggested", "Convert", "Compress"],
  );
  assert.deepEqual(ids(["image", "image"]), [
    "images_to_pdf",
    "image_compress",
    "image_resize",
    "image_convert",
    "apply_preset",
  ]);
});

test("one PDF suggests PDF to images, never merge", () => {
  assert.deepEqual(suggested(["pdf"]), ["pdf_to_images"]);
  assert.ok(!ids(["pdf"]).includes("pdf_merge"));
});

test("two PDFs suggest merge, and PDF to images needs exactly one", () => {
  assert.deepEqual(suggested(["pdf", "pdf"]), ["pdf_merge"]);
  assert.ok(!ids(["pdf", "pdf"]).includes("pdf_to_images"));
});

test("mixed images and PDFs are incompatible with per-type groups", () => {
  const menu = buildMenu(["image", "pdf", "image"]);
  assert.equal(menu.incompatible, true);
  assert.deepEqual(
    menu.sections.map((s) => [s.title, s.scope]),
    [
      ["For the 2 images", "image"],
      ["For the 1 PDF", "pdf"],
    ],
  );
  assert.deepEqual(
    menu.sections[1].actions.map((a) => a.id),
    ["pdf_to_images"],
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
  assert.deepEqual(search("passport"), ["apply_preset"]);
  assert.deepEqual(search("into pdf"), ["images_to_pdf"]);
  assert.deepEqual(search("cmprs"), ["image_compress"]);
  assert.deepEqual(search("webp"), ["image_convert"]);
  assert.deepEqual(search("  "), flatActions(menu).map((a) => a.id));
  assert.deepEqual(search("zzz"), []);
});

test("every action has a unique id", () => {
  assert.equal(new Set(ACTIONS.map((a) => a.id)).size, ACTIONS.length);
});
