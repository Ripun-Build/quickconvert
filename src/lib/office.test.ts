import { test } from "node:test";
import assert from "node:assert/strict";
import { byType, chooseFor, convertLabel, docType, enginesFor, fontsNote, handles, unhandled, type EngineInfo } from "./office.ts";
import type { FileInfo } from "./types.ts";

const file = (name: string): FileInfo => ({ path: `/p/${name}`, name, kind: "document", size: 1 });
const infos = (...installed: EngineInfo["engine"][]): EngineInfo[] =>
  (["pages", "keynote", "numbers", "libreoffice"] as const).map((engine) => ({
    engine,
    installed: installed.includes(engine),
    version: null,
    path: null,
  }));

test("file types and labels follow the selection", () => {
  assert.equal(docType("Report.DOCX"), "word");
  assert.equal(docType("deck.key"), "slides");
  assert.equal(docType("data.csv"), "sheet");
  assert.equal(docType("photo.jpg"), null);
  assert.equal(convertLabel([file("a.docx"), file("b.rtf")]), "Word to PDF");
  assert.equal(convertLabel([file("a.pptx")]), "PowerPoint to PDF");
  assert.equal(convertLabel([file("a.docx"), file("b.xlsx")]), "Convert to PDF");
  assert.deepEqual(
    byType([file("b.xlsx"), file("a.docx"), file("c.pptx")]).map((g) => g.type),
    ["word", "slides", "sheet"],
  );
});

test("each type shows the ways that can make it; PowerPoint has no Quick", () => {
  assert.deepEqual(enginesFor([file("a.docx")]), ["quick", "pages", "libreoffice"]);
  assert.deepEqual(enginesFor([file("a.pptx")]), ["keynote", "libreoffice"]);
  assert.deepEqual(enginesFor([file("a.numbers")]), ["numbers", "libreoffice"]);
  assert.ok(!handles("quick", "a.pages"));
});

test("nothing installed: Word and Excel go Quick, PowerPoint has nothing ready", () => {
  assert.equal(chooseFor([file("a.docx")], null, infos()), "quick");
  assert.equal(chooseFor([file("a.xlsx")], null, infos()), "quick");
  assert.equal(chooseFor([file("a.pptx")], null, infos()), null);
});

test("installed apps win unless Quick was picked; a pick that's gone falls back", () => {
  assert.equal(chooseFor([file("a.docx")], null, infos("pages")), "pages");
  assert.equal(chooseFor([file("a.docx")], "quick", infos("pages")), "quick");
  assert.equal(chooseFor([file("a.pptx")], "keynote", infos("libreoffice")), "libreoffice");
  assert.equal(chooseFor([file("a.pptx")], null, infos("keynote", "libreoffice")), "keynote");
});

test("files the chosen way can't make are pointed out", () => {
  const files = [file("a.docx"), file("b.pages")];
  assert.deepEqual(
    unhandled(files, "quick").map((f) => f.name),
    ["b.pages"],
  );
  assert.deepEqual(unhandled(files, null).length, 2);
});

test("the Fonts row says what happens for each way", () => {
  assert.match(fontsNote("quick", "sheet"), /standard font/);
  assert.match(fontsNote("quick", "word"), /nothing needs downloading/);
  assert.match(fontsNote("libreoffice", "slides"), /Carlito and Caladea/);
  assert.doesNotMatch(fontsNote("keynote", "slides"), /Carlito/);
});
