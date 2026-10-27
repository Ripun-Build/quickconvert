// Action registry, suggestion logic and option schemas (SPECS §6). Pure module (no UI imports) so it runs
// under `node --test`.
import type { JobRequest, ResizeMode } from "./types";

export type InputKind = "image" | "pdf" | "document";

export type ActionId =
  | "images_to_pdf"
  | "pdf_to_images"
  | "image_convert"
  | "image_resize"
  | "image_crop"
  | "image_rotate"
  | "strip_metadata"
  | "image_compress"
  | "pdf_merge"
  | "office_to_pdf"
  | "pdf_split"
  | "pdf_rotate"
  | "pdf_compress"
  | "apply_preset";

export type ActionGroup = "convert" | "edit" | "compress";

export interface ActionDef {
  id: ActionId;
  label: string;
  group: ActionGroup;
  /** Input kind every file the action runs on must have. */
  input: InputKind;
  minFiles: number;
  maxFiles?: number;
  keywords: string[];
}

export const ACTIONS: readonly ActionDef[] = [
  {
    id: "images_to_pdf",
    label: "Combine into PDF",
    group: "convert",
    input: "image",
    minFiles: 1,
    keywords: ["pdf", "merge", "join", "document", "scan"],
  },
  {
    id: "pdf_to_images",
    label: "PDF to JPG/PNG",
    group: "convert",
    input: "pdf",
    minFiles: 1,
    maxFiles: 1,
    keywords: ["jpg", "jpeg", "png", "image", "pages", "export"],
  },
  {
    id: "image_convert",
    label: "Convert to…",
    group: "convert",
    input: "image",
    minFiles: 1,
    keywords: ["jpg", "jpeg", "png", "webp", "heic", "format"],
  },
  {
    id: "pdf_merge",
    label: "Merge PDFs",
    group: "convert",
    input: "pdf",
    minFiles: 2,
    keywords: ["combine", "join", "pdf"],
  },
  {
    id: "image_resize",
    label: "Resize",
    group: "edit",
    input: "image",
    minFiles: 1,
    keywords: ["scale", "dimensions", "pixels", "smaller"],
  },
  {
    id: "image_crop",
    label: "Crop",
    group: "edit",
    input: "image",
    minFiles: 1,
    maxFiles: 1,
    keywords: ["trim", "cut", "passport", "square", "aspect", "rotate"],
  },
  {
    id: "image_rotate",
    label: "Rotate",
    group: "edit",
    input: "image",
    minFiles: 1,
    keywords: ["turn", "sideways", "upright", "flip", "orientation"],
  },
  {
    id: "strip_metadata",
    label: "Remove location & camera info",
    group: "edit",
    input: "image",
    minFiles: 1,
    keywords: ["privacy", "metadata", "exif", "gps", "location", "camera", "strip", "clean", "private"],
  },
  {
    id: "office_to_pdf",
    label: "Convert to PDF",
    group: "convert",
    input: "document",
    minFiles: 1,
    keywords: [
      "pdf",
      "word",
      "docx",
      "doc",
      "powerpoint",
      "pptx",
      "slides",
      "excel",
      "xlsx",
      "csv",
      "spreadsheet",
      "keynote",
      "pages",
      "numbers",
    ],
  },
  {
    id: "pdf_split",
    label: "Split PDF",
    group: "edit",
    input: "pdf",
    minFiles: 1,
    maxFiles: 1,
    keywords: ["cut", "separate", "extract", "pages", "keep", "remove", "pick"],
  },
  {
    id: "pdf_rotate",
    label: "Rotate pages",
    group: "edit",
    input: "pdf",
    minFiles: 1,
    maxFiles: 1,
    keywords: ["turn", "sideways", "upright", "orientation", "straighten"],
  },
  {
    id: "image_compress",
    label: "Compress",
    group: "compress",
    input: "image",
    minFiles: 1,
    keywords: ["shrink", "size", "kb", "reduce", "smaller"],
  },
  {
    id: "pdf_compress",
    label: "Make PDF smaller",
    group: "compress",
    input: "pdf",
    minFiles: 1,
    keywords: ["compress", "shrink", "reduce", "size", "email", "mb"],
  },
  {
    id: "apply_preset",
    label: "Apply preset…",
    group: "compress",
    input: "image",
    minFiles: 1,
    keywords: ["exam", "passport", "photo", "signature", "portal", "whatsapp", "email"],
  },
];

const byId = (id: ActionId): ActionDef => {
  const action = ACTIONS.find((a) => a.id === id);
  if (!action) throw new Error(`unknown action ${id}`);
  return action;
};

/** Suggested order per selection shape (SPECS §6). */
const SUGGESTED: Record<"oneImage" | "manyImages" | "onePdf" | "manyPdfs" | "documents", ActionId[]> = {
  oneImage: ["image_convert", "image_compress", "image_resize", "images_to_pdf"],
  manyImages: ["images_to_pdf", "image_compress", "image_resize"],
  onePdf: ["pdf_to_images", "pdf_compress", "pdf_split"],
  manyPdfs: ["pdf_merge", "pdf_compress"],
  documents: ["office_to_pdf"],
};

const GROUP_TITLES: Record<ActionGroup, string> = {
  convert: "Convert",
  edit: "Edit",
  compress: "Compress",
};
const GROUP_ORDER: ActionGroup[] = ["convert", "edit", "compress"];

export interface MenuItem {
  action: ActionDef;
  /** False when the action doesn't fit the selection; it's shown greyed out with `reason`. */
  enabled: boolean;
  reason?: string;
}

export interface MenuSection {
  title: string;
  items: MenuItem[];
  /** Set when the section only applies to the files of this kind (mixed selections). */
  scope?: InputKind;
}

export interface Menu {
  sections: MenuSection[];
  /** Mixed selection with no action valid for every file. */
  incompatible: boolean;
}

/** Whether `action` can run on `count` files that are all of `kind`. */
export function accepts(action: ActionDef, kind: InputKind, count: number): boolean {
  return action.input === kind && count >= action.minFiles && count <= (action.maxFiles ?? Infinity);
}

const NOUNS: Record<InputKind, [string, string]> = {
  image: ["image", "images"],
  pdf: ["PDF", "PDFs"],
  document: ["document", "documents"],
};
const noun = (kind: InputKind, many: boolean) => NOUNS[kind][many ? 1 : 0];

/** Short explanation of why `action` can't run on `count` files of `kind`. */
export function unavailableReason(action: ActionDef, kind: InputKind, count: number): string {
  const want = action.input;
  if (want !== kind || count < action.minFiles) {
    if (action.minFiles > 1) return `Needs ${action.minFiles}+ ${noun(want, true)}`;
    if (action.maxFiles === 1) return `Needs one ${noun(want, false)}`;
    return want === "pdf" ? "Needs a PDF" : want === "document" ? "Needs Word, PowerPoint or Excel files" : "Needs images";
  }
  return `Works on one ${noun(want, false)} at a time`;
}

const item = (action: ActionDef, kind: InputKind, count: number): MenuItem =>
  accepts(action, kind, count)
    ? { action, enabled: true }
    : { action, enabled: false, reason: unavailableReason(action, kind, count) };

/** Enabled items first, keeping registry order within each half. */
const enabledFirst = (items: MenuItem[]) => [...items.filter((i) => i.enabled), ...items.filter((i) => !i.enabled)];

/**
 * Menu for a selection: Suggested first, then every other action by group. Actions that don't fit
 * the selection stay visible but disabled, with a reason.
 */
export function buildMenu(kinds: readonly InputKind[]): Menu {
  if (kinds.length === 0) return { sections: [], incompatible: false };

  const order: InputKind[] = ["image", "pdf", "document"];
  const counts = new Map(order.map((k) => [k, kinds.filter((x) => x === k).length]));
  const present = order.filter((k) => (counts.get(k) ?? 0) > 0);
  if (present.length > 1) {
    // No action takes two kinds of file at once, so offer a group per kind instead.
    const groups = present.map((kind) => {
      const n = counts.get(kind) ?? 0;
      const actions = ACTIONS.filter((a) => accepts(a, kind, n));
      return { kind, n, actions };
    });
    const neither = ACTIONS.filter((a) => !groups.some((g) => g.actions.includes(a)));
    const sections: MenuSection[] = [
      ...groups.map((g) => ({
        title: `For the ${g.n} ${noun(g.kind, g.n !== 1)}`,
        scope: g.kind,
        items: g.actions.map((a) => item(a, g.kind, g.n)),
      })),
      { title: "Not available", items: neither.map((a) => item(a, a.input, counts.get(a.input) ?? 0)) },
    ];
    return { incompatible: true, sections: sections.filter((s) => s.items.length > 0) };
  }

  const kind = present[0];
  const count = kinds.length;
  const shape =
    kind === "document"
      ? "documents"
      : kind === "image"
        ? count === 1
          ? "oneImage"
          : "manyImages"
        : count === 1
          ? "onePdf"
          : "manyPdfs";
  const suggested = SUGGESTED[shape].map(byId).filter((a) => accepts(a, kind, count));
  const rest = ACTIONS.filter((a) => !suggested.includes(a));

  const sections: MenuSection[] = [{ title: "Suggested", items: suggested.map((a) => item(a, kind, count)) }];
  for (const group of GROUP_ORDER) {
    const items = rest.filter((a) => a.group === group).map((a) => item(a, kind, count));
    sections.push({ title: GROUP_TITLES[group], items: enabledFirst(items) });
  }
  return { sections: sections.filter((s) => s.items.length > 0), incompatible: false };
}

/** The same menu with one action shown under another name (e.g. "Word to PDF" for Convert to PDF). */
export function withLabel(menu: Menu, id: ActionId, label: string): Menu {
  return {
    ...menu,
    sections: menu.sections.map((s) => ({
      ...s,
      items: s.items.map((i) => (i.action.id === id ? { ...i, action: { ...i.action, label } } : i)),
    })),
  };
}

/** Filter a menu by a typed query (see `matches`). Empty sections are dropped. */
export function filterMenu(menu: Menu, query: string): Menu {
  const q = query.trim().toLowerCase();
  if (!q) return menu;
  return {
    ...menu,
    sections: menu.sections
      .map((s) => ({ ...s, items: s.items.filter((i) => matches(i.action, q)) }))
      .filter((s) => s.items.length > 0),
  };
}

/**
 * Every typed word must match a label word or keyword: as a prefix ("comp" → Compress), or loosely
 * when 3+ letters share the first letter and appear in order ("cmprs" → Compress).
 */
export function matches(action: ActionDef, query: string): boolean {
  const words = [...action.label.toLowerCase().split(/[^a-z0-9]+/), ...action.keywords].filter(Boolean);
  const tokens = query.toLowerCase().split(/\s+/).filter(Boolean);
  return tokens.every((token) => words.some((word) => wordMatches(token, word)));
}

function wordMatches(token: string, word: string): boolean {
  if (word.startsWith(token)) return true;
  return token.length >= 3 && token[0] === word[0] && isSubsequence(token, word);
}

function isSubsequence(needle: string, haystack: string): boolean {
  let i = 0;
  for (const ch of haystack) {
    if (ch === needle[i]) i++;
    if (i === needle.length) return true;
  }
  return false;
}

/** Runnable actions in display order; index + 1 is the 1–9 shortcut. */
export function flatActions(menu: Menu): ActionDef[] {
  return menu.sections.flatMap((s) => s.items.filter((i) => i.enabled).map((i) => i.action));
}

// ---- Options (SPECS §6 defaults) ----

export type OptionValues = Record<string, string | number | boolean>;

interface FieldBase {
  key: string;
  label: string;
  /** Hide the field unless this returns true for the current values. */
  visible?: (values: OptionValues) => boolean;
}

export type OptionField =
  | (FieldBase & { kind: "segmented"; choices: { value: string; label: string }[] })
  | (FieldBase & { kind: "slider"; min: number; max: number; step: number; unit?: string })
  | (FieldBase & { kind: "number"; min: number; max: number; unit: string })
  | (FieldBase & { kind: "toggle" })
  | (FieldBase & { kind: "text"; placeholder: string; valid: (value: string) => boolean; hint: string });

export interface ActionSpec {
  fields: OptionField[];
  defaults: OptionValues;
  /** Primary button label. */
  submit: string;
  toRequest: (values: OptionValues) => JobRequest;
}

const str = (v: OptionValues[string]) => String(v);
const num = (v: OptionValues[string]) => Number(v);

/** Page list like "1-3, 5". Syntax only; Rust checks it against the page count. */
export const PAGE_RANGE = /^\s*\d+\s*(-\s*\d+\s*)?(,\s*\d+\s*(-\s*\d+\s*)?)*,?\s*$/;

/** Actions that can run. Resize, crop, rotate, PDF → images, Compress and presets use their own editor views. */
export const ACTION_SPECS: Partial<Record<ActionId, ActionSpec>> = {
  images_to_pdf: {
    submit: "Create PDF",
    defaults: { pageSize: "fit", margin: "none" },
    fields: [
      {
        kind: "segmented",
        key: "pageSize",
        label: "Page size",
        choices: [
          { value: "fit", label: "Fit to image" },
          { value: "a4", label: "A4" },
          { value: "letter", label: "Letter" },
        ],
      },
      {
        kind: "segmented",
        key: "margin",
        label: "Margin",
        choices: [
          { value: "none", label: "None" },
          { value: "small", label: "Small" },
        ],
      },
    ],
    toRequest: (v) => ({
      action: "images_to_pdf",
      pageSize: str(v.pageSize) as "fit" | "a4" | "letter",
      margin: str(v.margin) as "none" | "small",
    }),
  },
  image_compress: {
    submit: "Compress",
    defaults: {},
    fields: [],
    toRequest: () => ({ action: "image_compress", target: { kind: "size", maxKb: 200 } }),
  },
  apply_preset: {
    submit: "Apply preset",
    defaults: {},
    fields: [],
    // The preset view supplies the chosen preset.
    toRequest: () => {
      throw new Error("apply_preset needs a preset");
    },
  },
  image_crop: {
    submit: "Crop",
    defaults: {},
    fields: [],
    // The crop editor supplies the box and turn.
    toRequest: () => {
      throw new Error("image_crop needs a crop box");
    },
  },
  image_rotate: {
    submit: "Rotate",
    defaults: {},
    fields: [],
    // The rotate view supplies the direction.
    toRequest: () => {
      throw new Error("image_rotate needs a direction");
    },
  },
  office_to_pdf: {
    submit: "Convert to PDF",
    defaults: {},
    fields: [],
    // The Convert to PDF screen supplies the ways and options.
    toRequest: () => {
      throw new Error("office_to_pdf needs engines");
    },
  },
  pdf_split: {
    submit: "Split",
    defaults: {},
    fields: [],
    // The split editor supplies the parts or pages.
    toRequest: () => {
      throw new Error("pdf_split needs parts");
    },
  },
  pdf_rotate: {
    submit: "Rotate",
    defaults: {},
    fields: [],
    toRequest: () => {
      throw new Error("pdf_rotate needs turns");
    },
  },
  pdf_compress: {
    submit: "Make smaller",
    defaults: {},
    fields: [],
    toRequest: () => {
      throw new Error("pdf_compress needs a level");
    },
  },
  strip_metadata: {
    submit: "Remove info",
    defaults: {},
    fields: [],
    toRequest: () => ({ action: "strip_metadata" }),
  },
  pdf_merge: {
    submit: "Merge",
    defaults: {},
    fields: [],
    toRequest: () => ({ action: "pdf_merge" }),
  },
  pdf_to_images: {
    submit: "Export pages",
    defaults: { format: "jpg", dpi: "150", pages: "all", range: "" },
    fields: [
      {
        kind: "segmented",
        key: "format",
        label: "Format",
        choices: [
          { value: "jpg", label: "JPG" },
          { value: "png", label: "PNG" },
        ],
      },
      {
        kind: "segmented",
        key: "dpi",
        label: "Resolution",
        choices: [
          { value: "72", label: "72 DPI" },
          { value: "150", label: "150 DPI" },
          { value: "300", label: "300 DPI" },
        ],
      },
      {
        kind: "segmented",
        key: "pages",
        label: "Pages",
        choices: [
          { value: "all", label: "All" },
          { value: "range", label: "Range" },
        ],
      },
      {
        kind: "text",
        key: "range",
        label: "Page range",
        placeholder: "1-3, 5",
        hint: "Pages and ranges separated by commas, like 1-3, 5",
        valid: (v) => PAGE_RANGE.test(v) && /\d/.test(v),
        visible: (v) => v.pages === "range",
      },
    ],
    toRequest: (v) => ({
      action: "pdf_to_images",
      format: str(v.format) as "jpg" | "png",
      dpi: num(v.dpi),
      pages: v.pages === "range" ? { kind: "range", spec: str(v.range) } : { kind: "all" },
    }),
  },
  image_convert: {
    submit: "Convert",
    defaults: { format: "jpg", quality: 85 },
    fields: [
      {
        kind: "segmented",
        key: "format",
        label: "Format",
        choices: [
          { value: "jpg", label: "JPG" },
          { value: "png", label: "PNG" },
          { value: "webp", label: "WebP" },
        ],
      },
      { kind: "slider", key: "quality", label: "Quality", min: 1, max: 100, step: 1, visible: (v) => v.format !== "png" },
    ],
    toRequest: (v) => ({
      action: "image_convert",
      format: str(v.format) as "jpg" | "png" | "webp",
      quality: num(v.quality),
    }),
  },
  image_resize: {
    submit: "Resize",
    defaults: { mode: "long_edge", px: 1600, width: 1600, height: 1600, keepAspect: true, percent: 50 },
    fields: [
      {
        kind: "segmented",
        key: "mode",
        label: "Resize by",
        choices: [
          { value: "long_edge", label: "Long edge" },
          { value: "exact", label: "Exact size" },
          { value: "percent", label: "Percent" },
        ],
      },
      { kind: "number", key: "px", label: "Long edge", min: 1, max: 20000, unit: "px", visible: (v) => v.mode === "long_edge" },
      { kind: "number", key: "width", label: "Width", min: 1, max: 20000, unit: "px", visible: (v) => v.mode === "exact" },
      { kind: "number", key: "height", label: "Height", min: 1, max: 20000, unit: "px", visible: (v) => v.mode === "exact" },
      { kind: "toggle", key: "keepAspect", label: "Keep aspect ratio", visible: (v) => v.mode === "exact" },
      { kind: "slider", key: "percent", label: "Scale", min: 1, max: 200, step: 1, unit: "%", visible: (v) => v.mode === "percent" },
    ],
    toRequest: (v) => {
      const mode: ResizeMode =
        v.mode === "exact"
          ? { kind: "exact", width: num(v.width), height: num(v.height), keepAspect: Boolean(v.keepAspect) }
          : v.mode === "percent"
            ? { kind: "percent", percent: num(v.percent) }
            : { kind: "long_edge", px: num(v.px) };
      return { action: "image_resize", mode };
    },
  },
};
