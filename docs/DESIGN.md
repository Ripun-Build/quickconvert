# QuickConvert — Design System

The popup should feel like a native macOS HUD / Spotlight panel: translucent, calm, quick, and quiet. Principles:

1. **Restraint.** One accent colour, few borders, generous spacing, no decorative gradients.
2. **Depth through material.** Blurred translucent background + soft shadow, not heavy outlines.
3. **Physical motion.** Springs, not linear easing. Things move from where they came from.
4. **Immediate feedback.** Every press, hover and keypress responds within one frame.
5. **Clarity over density.** Show only the actions that make sense for the selected files.

## 1. Tokens (`src/styles/tokens.css`)

All colours, radii, spacing and type sizes come from these CSS variables. Tailwind's theme maps to them (e.g. `bg-surface`, `text-secondary`, `rounded-popup`). Never hard-code values in components.

```css
:root {
  /* Material */
  --bg-glass: rgba(246, 246, 248, 0.72);   /* over native vibrancy */
  --bg-solid: #F5F5F7;                      /* fallback when blur unavailable */
  --surface: rgba(255, 255, 255, 0.60);     /* cards, inputs */
  --surface-hover: rgba(0, 0, 0, 0.05);
  --surface-selected: rgba(0, 122, 255, 0.12);
  --surface-raised: #FFFFFF;                 /* segmented thumb, slider knob */
  --separator: rgba(0, 0, 0, 0.08);
  --border-glass: rgba(255, 255, 255, 0.55); /* 1px inner highlight */

  /* Text */
  --text-primary: rgba(0, 0, 0, 0.88);
  --text-secondary: rgba(0, 0, 0, 0.56);
  --text-tertiary: rgba(0, 0, 0, 0.36);
  --text-on-accent: #FFFFFF;

  /* Accent & status */
  --accent: #007AFF;
  --accent-pressed: #0062CC;
  --success: #34C759;
  --warning: #FF9F0A;
  --danger: #FF3B30;
  --focus-ring: rgba(0, 122, 255, 0.45);

  /* Shape */
  --radius-popup: 18px;
  --radius-card: 12px;
  --radius-row: 8px;
  --radius-control: 7px;
  --radius-thumb: 6px;
  --radius-kbd: 5px;
  --radius-pill: 999px;

  /* Elevation */
  --shadow-popup: 0 0 0 0.5px rgba(0,0,0,0.12), 0 12px 40px rgba(0,0,0,0.18), 0 2px 8px rgba(0,0,0,0.08);

  --shadow-control: 0 0.5px 2px rgba(0,0,0,0.25), 0 0 0 0.5px rgba(0,0,0,0.06); /* knobs, thumbs */

  /* Spacing scale (4 px base) */
  --space-1: 4px; --space-2: 8px; --space-3: 12px; --space-4: 16px; --space-5: 20px; --space-6: 24px;

  /* Type */
  --font-sans: -apple-system, BlinkMacSystemFont, "SF Pro Text", "Segoe UI Variable Text", "Segoe UI", Inter, system-ui, sans-serif;
  --font-mono: ui-monospace, "SF Mono", "Cascadia Mono", Consolas, monospace;
  --text-title: 600 15px/20px var(--font-sans);
  --text-body: 400 13px/18px var(--font-sans);
  --text-body-strong: 500 13px/18px var(--font-sans);
  --text-caption: 400 11px/14px var(--font-sans);
  --tracking-tight: -0.01em;
}

@media (prefers-color-scheme: dark) {
  :root:not([data-theme="light"]) {
    --bg-glass: rgba(30, 30, 32, 0.70);
    --bg-solid: #1C1C1E;
    --surface: rgba(255, 255, 255, 0.06);
    --surface-hover: rgba(255, 255, 255, 0.08);
    --surface-selected: rgba(10, 132, 255, 0.24);
    --surface-raised: rgba(255, 255, 255, 0.14);
    --separator: rgba(255, 255, 255, 0.10);
    --border-glass: rgba(255, 255, 255, 0.10);
    --text-primary: rgba(255, 255, 255, 0.92);
    --text-secondary: rgba(255, 255, 255, 0.60);
    --text-tertiary: rgba(255, 255, 255, 0.38);
    --accent: #0A84FF;
    --accent-pressed: #0070E0;
    --success: #30D158;
    --warning: #FF9F0A;
    --danger: #FF453A;
    --focus-ring: rgba(10, 132, 255, 0.55);
    --shadow-popup: 0 0 0 0.5px rgba(255,255,255,0.10), 0 16px 48px rgba(0,0,0,0.50);
  }
}
:root[data-theme="dark"] {
  /* Copy every variable from the dark media block above (forced dark mode). */
}
```

Settings → Appearance sets `data-theme="light" | "dark"` on `<html>`, or removes it for System.

## 2. Typography

- Title (popup header, sheet titles): `--text-title`, tracking `--tracking-tight`.
- Body (action rows, buttons, inputs): `--text-body` / `--text-body-strong`.
- Caption (file sizes, hints, shortcut numbers): `--text-caption`, `--text-secondary`.
- Numbers in progress/sizes use `font-variant-numeric: tabular-nums`.
- Sentence case everywhere ("Combine into PDF", not "Combine Into PDF"). No exclamation marks.

## 3. Layout

```
┌──────────────────────────────────────┐  360 px wide, radius 18
│ [thumb] holiday.jpg        2.4 MB  ⚙ │  header 52 px, padding 12/16
│ ┌──────────────────────────────────┐ │
│ │ 🔍 Type an action…               │ │  search 32 px, radius 7
│ └──────────────────────────────────┘ │
│ SUGGESTED                            │  section label: caption, tertiary, uppercase, tracking 0.04em
│ ▸ 📄 Combine into PDF            1   │  row 36 px, radius 8, icon 18 px
│   🗜 Compress                     2   │
│   ↔ Resize                       3   │
│ CONVERT                              │
│   🖼 Convert to…                  4   │
│ ──────────────────────────────────── │
│ ↑↓ navigate · ↵ select · esc close   │  footer hint 28 px, caption, tertiary
└──────────────────────────────────────┘
```

- Outer padding 8 px around the list; rows have 10 px horizontal padding.
- Selected/highlighted row: `--surface-selected` background, icon tinted `--accent`.
- Hover: `--surface-hover`. Keyboard highlight and mouse hover are the same single highlight (mouse move updates it).
- Shortcut numbers shown as `Kbd` components (caption, tertiary, 18×18 rounded 5 px, `--surface` bg).
- Lists scroll inside the popup; header and footer stay fixed. Fade the list edges with a 12 px mask when scrollable.

## 4. Components

| Component | Spec |
|---|---|
| **FileChip** | 32 px thumbnail (radius 6, 0.5 px separator border) + name (body-strong, truncated middle: `IMG_20…_0412.jpg`) + size (caption). Multiple files: stacked 3-thumbnail fan (offset 4 px, rotate ±4°) + "12 images · 38 MB". |
| **ActionRow** | Icon (lucide, 18 px, stroke 1.75) + label + optional sublabel (caption) + Kbd. Height 36 px. Disabled: 40 % opacity, no hover/highlight, the reason (caption, tertiary) replaces the Kbd. |
| **Button** | Primary: `--accent` bg, `--text-on-accent`, height 30 px, radius 7, body-strong. Secondary: `--surface` bg, primary text. Pressed: scale 0.97 + `--accent-pressed`. Disabled: 40% opacity. |
| **Segmented** | Pill group, `--surface` track, selected segment white (light) / `rgba(255,255,255,0.14)` (dark) with subtle shadow; the thumb slides with a spring (layoutId). |
| **Slider** | 4 px track, 18 px white knob with shadow, live value in caption on the right (tabular). |
| **TextField** | 30 px, `--surface` bg, radius 7, focus ring 3 px `--focus-ring`. |
| **ProgressBar** | 4 px, radius pill, `--accent` fill on `--separator` track; indeterminate shimmer when percent unknown. |
| **Toast-style result** | Check icon draws in (stroke-dashoffset 280 ms), title "Saved", output name, three secondary buttons. |
| **PdfPagesEditor** | 720 px wide. Left: page grid (4 columns, `--surface` well, scrolls); selected pages get a 2 px `--accent` ring and a check badge, unselected ones fade to 55 %. Right column 232 px: Pages (All/None chips + range field, `--danger` ring and message when invalid), Format (Segmented + hint), Image size (three radio cards: label, pixel size, one-line description; selected = `--surface-selected` with `--accent` inset ring). |
| **ResizeEditor** | 720 px wide. Canvas on `--surface`, original size as a dashed `--separator` outline with the image at 15 % behind; live image on top. Handles: `--surface-raised` pills on the right/bottom edges, round corner handle with `--accent` border (keyboard: arrows 1 %, Shift 10 %). W × H label in a `--bg-glass` chip at the corner. Lock = Link/Unlink icon. |

Icons (lucide-react): FileText (PDF), Images, Image, Minimize2 (compress), Scaling (resize), Crop, RotateCw, Combine/Layers (merge), Scissors (split), SlidersHorizontal (options), Settings, Check, AlertTriangle, FolderOpen, Copy.

## 5. Motion (Framer Motion)

| Moment | Animation |
|---|---|
| Popup appear | From cursor origin: `scale 0.94 → 1`, `opacity 0 → 1`, `y 4 → 0`. Spring `{ stiffness: 520, damping: 34, mass: 0.8 }` (~180 ms). `transform-origin` = corner nearest the cursor. |
| Popup hide | `opacity → 0`, `scale → 0.97`, 120 ms ease-out. Then the native window is hidden. |
| View change (list → options) | Push: new view slides in from the right 24 px + fade; old slides left 24 px + fade. Back reverses direction. Spring `{ stiffness: 420, damping: 36 }`. |
| Height change | Animate container height with `layout` (spring), never jump. |
| Row highlight | Shared `layoutId` highlight pill glides between rows. |
| Expand to editor | Window resizes in Rust; content cross-fades; the thumbnail morphs (layoutId) into the editor preview. |
| Success | Check stroke draws, then subtle scale pop 1 → 1.06 → 1. |
| Error | Horizontal shake: x `[0, -6, 6, -4, 4, 0]` over 300 ms. |

**Reduced motion** (`prefers-reduced-motion: reduce`): replace all transforms with 120 ms opacity fades; no shake, no morph.

## 6. Material & window

- Window: transparent, borderless, **exactly the size of the visible popup**, with the OS window shadow on, always on top, skip taskbar.
- Native blur behind (`window-vibrancy`, rounded to `--radius-popup` = 18 px), then `--bg-glass` overlay in CSS, plus a 1 px inner highlight using `--border-glass` (`box-shadow: inset 0 0 0 1px var(--border-glass)`).
- If vibrancy is unavailable, set `data-blur="off"` on `<html>` and use `--bg-solid`.
- Why no CSS shadow: the native blur view fills the whole window, so a transparent 24 px margin for a CSS shadow would show as a blurred frame. `--shadow-popup` is kept for in-page elevated surfaces only (decided in M1).

## 7. Copy & tone

- Short, plain, friendly. Verbs first: "Combine into PDF", "Compress to 50 KB".
- Results state facts: "Saved · 48 KB · 200×230".
- Errors say what happened and what to do, never blame: "This PDF is password-protected."

## 8. Accessibility

- Contrast ≥ 4.5:1 for body text on `--bg-solid` in both themes; check secondary text too.
- All interactive elements have visible `:focus-visible` rings.
- ARIA: popup root `role="dialog"` `aria-label="QuickConvert"`; action list is `role="listbox"` with `aria-activedescendant`.
- Hit targets ≥ 28 px tall.
