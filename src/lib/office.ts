// Convert to PDF for Word / PowerPoint / Excel: file types, the ways to make the PDF, and what to
// tell people about each way. Pure, so it runs under `node --test`. Mirrors src-tauri/src/office.
import type { Engine, EngineInfo, FileInfo } from "./types";

export type { Engine, EngineInfo };
export type DocType = "word" | "slides" | "sheet";

export const DOC_TYPES: DocType[] = ["word", "slides", "sheet"];

const EXTENSIONS: Record<DocType, string[]> = {
  word: ["docx", "doc", "rtf", "odt", "pages"],
  slides: ["pptx", "ppt", "key", "odp"],
  sheet: ["xlsx", "xls", "xlsm", "xlsb", "ods", "csv", "numbers"],
};

const ext = (name: string) => name.slice(name.lastIndexOf(".") + 1).toLowerCase();

export function docType(name: string): DocType | null {
  const e = ext(name);
  return DOC_TYPES.find((t) => EXTENSIONS[t].includes(e)) ?? null;
}

/** What people call each type. */
export const TYPE_LABEL: Record<DocType, string> = { word: "Word", slides: "PowerPoint", sheet: "Excel" };

/** Apple's app for each type. */
export const APPLE_APP: Record<DocType, Engine> = { word: "pages", slides: "keynote", sheet: "numbers" };

/** Apps in the order they're preferred when nothing was picked (same as Rust `office::APPS`). */
export const APPS: Engine[] = ["pages", "keynote", "numbers", "libreoffice"];

export const ENGINE_NAME: Record<Engine, string> = {
  quick: "Quick (built in)",
  pages: "Pages",
  keynote: "Keynote",
  numbers: "Numbers",
  libreoffice: "LibreOffice",
};

/** Whether `engine` can make a PDF of this file (mirrors `Engine::handles`). */
export function handles(engine: Engine, name: string): boolean {
  const e = ext(name);
  switch (engine) {
    case "quick":
      return ["docx", "doc", "rtf", "odt", "xlsx", "xls", "xlsm", "xlsb", "ods", "csv"].includes(e);
    case "pages":
      return ["docx", "doc", "rtf", "pages"].includes(e);
    case "keynote":
      return ["pptx", "ppt", "key"].includes(e);
    case "numbers":
      return ["xlsx", "xls", "csv", "numbers"].includes(e);
    case "libreoffice":
      return docType(name) !== null;
  }
}

/** The ways shown for a type's files: those that can make at least one of them, Quick first. */
export function enginesFor(files: readonly FileInfo[]): Engine[] {
  return (["quick", ...APPS] as Engine[]).filter((e) => files.some((f) => handles(e, f.name)));
}

const installed = (engine: Engine, infos: readonly EngineInfo[]) =>
  engine === "quick" || infos.some((i) => i.engine === engine && i.installed);

/**
 * The way to use for a type's files: the remembered pick if it's ready, otherwise the best installed
 * app, otherwise Quick, otherwise null (nothing ready: an app has to be installed).
 */
export function chooseFor(files: readonly FileInfo[], picked: Engine | null | undefined, infos: readonly EngineInfo[]): Engine | null {
  const ready = (e: Engine) => installed(e, infos) && files.some((f) => handles(e, f.name));
  if (picked && ready(picked)) return picked;
  return [...APPS, "quick" as Engine].find(ready) ?? null;
}

/** Files of a type that the chosen way can't make (they'd fall back, or need an app). */
export const unhandled = (files: readonly FileInfo[], engine: Engine | null) =>
  engine ? files.filter((f) => !handles(engine, f.name)) : [...files];

/** The action's name for a selection: "Word to PDF", "Excel to PDF", or "Convert to PDF" for a mix. */
export function convertLabel(files: readonly FileInfo[]): string {
  const types = new Set(files.map((f) => docType(f.name)).filter(Boolean) as DocType[]);
  if (types.size === 1) return `${TYPE_LABEL[[...types][0]]} to PDF`;
  return "Convert to PDF";
}

/** Group a selection's files by type, in Word / PowerPoint / Excel order. */
export function byType(files: readonly FileInfo[]): { type: DocType; files: FileInfo[] }[] {
  return DOC_TYPES.map((type) => ({ type, files: files.filter((f) => docType(f.name) === type) })).filter((g) => g.files.length > 0);
}

// ---- What the ⓘ panel says ----

export interface Details {
  title: string;
  what: string;
  /** Why it's better, or (Quick) what changes compared with the original. */
  why: Partial<Record<DocType, string>> & { all?: string };
  from: string;
  size: string;
  /** How Install works for it. */
  install: "none" | "store" | "download";
}

export const DETAILS: Record<Engine, Details> = {
  quick: {
    title: "Quick (built in)",
    what: "Uses the text engine that's part of macOS. Nothing to install, and it's ready now.",
    why: {
      word: "Text, headings, bold and italic, lists and simple tables come out right. Columns, text boxes, headers and footers, footnotes and charts come out simpler, so it's not 100% the same as the original.",
      sheet: "Every value from every sheet, in a clean table fitted to the page. Cell colours, borders, charts and Excel's number formats aren't kept (15% shows as 0.15), so it's not 100% the same as the original.",
    },
    from: "Built into QuickConvert and macOS",
    size: "Nothing to download",
    install: "none",
  },
  pages: {
    title: "Pages",
    what: "Apple's free word processor for the Mac.",
    why: { all: "It understands Word documents fully, so the PDF looks exactly like the original: layout, fonts, headers and footers, pictures and tables." },
    from: "The Mac App Store · free, made by Apple",
    size: "About 0.5–1 GB",
    install: "store",
  },
  keynote: {
    title: "Keynote",
    what: "Apple's free presentation app for the Mac.",
    why: { all: "It understands PowerPoint slides fully, so the PDF shows every slide exactly as designed: pictures, shapes, layouts and fonts." },
    from: "The Mac App Store · free, made by Apple",
    size: "About 0.5–1 GB",
    install: "store",
  },
  numbers: {
    title: "Numbers",
    what: "Apple's free spreadsheet app for the Mac.",
    why: { all: "It understands Excel files fully, so the PDF keeps colours, borders, charts and number formats, laid out as Excel would print them." },
    from: "The Mac App Store · free, made by Apple",
    size: "About 0.5–1 GB",
    install: "store",
  },
  libreoffice: {
    title: "LibreOffice",
    what: "A free, open-source office suite that opens Word, PowerPoint and Excel files.",
    why: { all: "It makes exact PDFs of all three kinds of file, quietly in the background, without opening any windows." },
    from: "libreoffice.org · the official download, checked against LibreOffice's published checksum before it's installed",
    size: "", // filled in from the download for this Mac
    install: "download",
  },
};

/** The ⓘ panel's Fonts row. */
export function fontsNote(engine: Engine, type: DocType): string {
  if (engine === "quick" && type === "sheet") return "Tables are printed in the Mac's standard font.";
  const swap =
    "Fonts the file uses that aren't on this Mac are swapped for the closest ones that are (Avenir Next LT Pro → Avenir Next, Univers → Helvetica Neue), so nothing needs downloading. The result lists every swap.";
  return engine === "libreoffice"
    ? `${swap} LibreOffice also brings Carlito and Caladea, which match Calibri and Cambria letter for letter.`
    : swap;
}

/** "120 of 285 MB" */
export const megabytes = (bytes: number) => Math.round(bytes / 1_000_000);
