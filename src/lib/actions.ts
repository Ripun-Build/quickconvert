// Action registry and suggestion logic (SPECS §6). Pure module (no UI imports) so it runs under `node --test`.

export type InputKind = "image" | "pdf";

export type ActionId =
  | "images_to_pdf"
  | "pdf_to_images"
  | "image_convert"
  | "image_resize"
  | "image_compress"
  | "pdf_merge"
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
    keywords: ["jpg", "jpeg", "png", "webp", "format"],
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
    id: "image_compress",
    label: "Compress",
    group: "compress",
    input: "image",
    minFiles: 1,
    keywords: ["shrink", "size", "kb", "reduce", "smaller"],
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
const SUGGESTED: Record<"oneImage" | "manyImages" | "onePdf" | "manyPdfs", ActionId[]> = {
  oneImage: ["image_convert", "image_compress", "image_resize", "images_to_pdf"],
  manyImages: ["images_to_pdf", "image_compress", "image_resize"],
  onePdf: ["pdf_to_images"],
  manyPdfs: ["pdf_merge"],
};

const GROUP_TITLES: Record<ActionGroup, string> = {
  convert: "Convert",
  edit: "Edit",
  compress: "Compress",
};
const GROUP_ORDER: ActionGroup[] = ["convert", "edit", "compress"];

export interface MenuSection {
  title: string;
  actions: ActionDef[];
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

/** Menu for a selection: Suggested first, then the remaining valid actions by group. */
export function buildMenu(kinds: readonly InputKind[]): Menu {
  if (kinds.length === 0) return { sections: [], incompatible: false };

  const images = kinds.filter((k) => k === "image").length;
  const pdfs = kinds.length - images;
  if (images > 0 && pdfs > 0) {
    // No v1 action takes both images and PDFs, so offer per-type groups instead.
    return {
      incompatible: true,
      sections: [
        scopedSection("image", images, `For the ${plural(images, "image")}`),
        scopedSection("pdf", pdfs, `For the ${plural(pdfs, "PDF")}`),
      ].filter((s) => s.actions.length > 0),
    };
  }

  const kind: InputKind = images > 0 ? "image" : "pdf";
  const count = kinds.length;
  const shape = kind === "image" ? (count === 1 ? "oneImage" : "manyImages") : count === 1 ? "onePdf" : "manyPdfs";
  const suggested = SUGGESTED[shape].map(byId).filter((a) => accepts(a, kind, count));
  const rest = ACTIONS.filter((a) => accepts(a, kind, count) && !suggested.includes(a));

  const sections: MenuSection[] = [{ title: "Suggested", actions: suggested }];
  for (const group of GROUP_ORDER) {
    sections.push({ title: GROUP_TITLES[group], actions: rest.filter((a) => a.group === group) });
  }
  return { sections: sections.filter((s) => s.actions.length > 0), incompatible: false };
}

function scopedSection(kind: InputKind, count: number, title: string): MenuSection {
  return { title, scope: kind, actions: ACTIONS.filter((a) => accepts(a, kind, count)) };
}

function plural(n: number, word: string): string {
  return `${n} ${word}${n === 1 ? "" : "s"}`;
}

/** Filter a menu by a typed query (see `matches`). Empty sections are dropped. */
export function filterMenu(menu: Menu, query: string): Menu {
  const q = query.trim().toLowerCase();
  if (!q) return menu;
  return {
    ...menu,
    sections: menu.sections
      .map((s) => ({ ...s, actions: s.actions.filter((a) => matches(a, q)) }))
      .filter((s) => s.actions.length > 0),
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

/** Actions in display order; index + 1 is the 1–9 shortcut. */
export function flatActions(menu: Menu): ActionDef[] {
  return menu.sections.flatMap((s) => s.actions);
}
