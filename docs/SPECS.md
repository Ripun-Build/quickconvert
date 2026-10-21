# QuickConvert — Product Specification

Version 0.1 · Status: draft · Owner: Ripunjoy Brahma

## 1. Summary

QuickConvert is a lightweight desktop tray app for Windows and macOS. The user selects one or more files in File Explorer / Finder, presses a global hotkey, and a small, beautiful popup appears next to the mouse cursor offering the conversions that make sense for those files (PDF ↔ JPG, merge, compress, resize, crop…). One or two clicks later, the output file sits next to the original.

### Goals
- Turn "open converter app → drag file → choose format → choose folder → save" into **select → hotkey → click**.
- Apple-quality UI: glassy, calm, fast, fully keyboard-operable.
- 100% offline and private — files never leave the computer.
- Solve real pain: compress/resize to **exact size limits** required by exam, job and government portals.

### Non-goals (v1)
- Cloud processing, accounts, sync.
- Office document conversion (DOCX/PPTX → PDF).
- Mobile apps, browser extension.
- Editing PDF content (text, annotations).

## 2. Users

- Students and job applicants uploading documents/photos to portals with strict size/format rules.
- Anyone who regularly turns phone photos into PDFs, splits/merges PDFs, or shrinks images for email/WhatsApp.

## 3. Core user flow

1. User selects file(s) in Explorer/Finder.
2. User presses the hotkey.
3. Popup appears at the cursor within 150 ms, showing the selected files (chip) and suggested actions.
4. User clicks an action (or types to filter, ↑/↓ + Enter, or presses 1–9).
5. If the action has options, an inline options sheet slides in (sensible defaults pre-filled; Enter = run).
6. Progress shows inline. On success: result view with **Show in folder**, **Open**, **Copy path**; popup auto-hides after 2.5 s unless hovered.
7. Esc goes back one step, or closes the popup from the first view. Clicking outside closes it.

If no files are selected (or selection can't be read), the popup opens in **Drop Zone** state: "Drop files here" + a "Choose files…" button.

The selection is only read when Explorer/Finder is the frontmost app at the moment the hotkey is pressed; from any other app the popup opens in Drop Zone state. The popup appears immediately and the selection fills in when read, so a slow Explorer/Finder never delays it.

## 4. Triggers

| Trigger | Phase | Notes |
|---|---|---|
| Global hotkey | v1 | Default **Windows: `Ctrl+Alt+Space`**, **macOS: `⌃⇧Space`** (changed from ⌥⇧Space, which file-search tools commonly use). Configurable. If registration fails (conflict), show a tray notification and open Settings → Hotkey. |
| Tray icon menu | v1 | Items: Open QuickConvert · Settings… · Quit. "Open" shows the popup centred on the active monitor in Drop Zone state. |
| Drag & drop onto popup | v1 | Works in any popup state; replaces current selection. |
| Explorer/Finder right-click entry | Phase 3 | Windows shell context menu / macOS Quick Action. |

## 5. Popup behaviour

### Placement
- Top-left corner at cursor + (12 px, 12 px).
- If it would overflow the right/bottom edge of the cursor's monitor work area, flip to the left/above the cursor.
- Always clamp inside the work area with an 8 px margin. Correct on HiDPI and multi-monitor setups.

### Size
- Width 360 px. Height fits content, max 480 px (list scrolls inside).
- Editor views (Crop, Page Tray) expand to 720 × 520 px with a spring animation, re-clamped to the screen.

### States (views)
| View | Content |
|---|---|
| **DropZone** | Dashed rounded area, icon, "Drop files here", "Choose files…" button. |
| **ActionList** | Header: file chip (thumbnail, name or "N files", total size). Search field "Type an action…". Grouped list: *Suggested*, *Convert*, *Edit*, *Compress*. Each row: icon, label, shortcut number (1–9). **Every action is always listed**: ones that don't fit the selection are greyed out with a short reason ("Needs images", "Needs 2+ PDFs", "Works on one PDF at a time"), can't be chosen, and are skipped by ↑/↓ and 1–9. |
| **PdfPagesEditor** | Popup widens to 720 px. Page thumbnails (loaded as they scroll into view): click to select, Shift-click for a range; *All* / *None*; the "1-3, 5" field stays in sync both ways and explains mistakes. Format JPG / PNG with a one-line hint each. Image size as three described choices instead of DPI: **Screen** (72 DPI, viewing and sharing), **Standard** (150 DPI, most uses), **Print** (300 DPI, printing), each showing the page's pixel size. |
| **OptionsSheet** | Back chevron + action title. Only that action's options. Primary button "Convert" / "Compress" / etc. |
| **Running** | Per-batch progress bar + "3 of 12 · photo_03.jpg". Cancel button (Esc also cancels after confirm-free stop). |
| **Result** | Green check, output name(s), buttons: Show in folder · Open · Copy path. Auto-hide after 2.5 s unless hovered/focused. |
| **ErrorView** | Plain-language message, per-file failure list for batches ("10 done, 2 failed — Show details"), Retry / Back. |
| **ResizeEditor** | Popup widens to 720 px. Image preview inside a dashed outline of the original size; drag the corner or right/bottom edge to resize, live W × H label follows the corner. Width/Height fields stay in sync both ways; aspect lock (on); 25/50/75/100 % shortcuts. Never larger than the original. |
| **CropEditor** | Phase 2. Image preview, draggable crop box, aspect presets (Free, 1:1, 4:3, 3:4, 16:9, 35×45 mm passport), rotate 90°. |
| **PageTray** | Phase 2. Thumbnails of images/pages; drag to reorder, rotate, remove; then "Create PDF". |

### Keyboard
- Typing anywhere focuses the search field and filters actions (fuzzy match on label + keywords).
- ↑/↓ move highlight, Enter runs/opens options, 1–9 pick the Nth visible action (while the search field is empty).
- Esc: clears the search first, then back one view → close popup. Cmd/Ctrl+, opens Settings.
- Visible focus ring on all controls.

### Dismissal
- Hide on: Esc from first view, focus lost (click outside / app switch), successful auto-hide.
- Exception: the **Drop Zone** stays open when focus is lost, so files can be dragged in from Explorer/Finder. Esc closes it; a drop or file choice moves on to the action list.
- **Never** hide while a job is running because of focus loss — keep running and show a tray notification when done.

## 6. Action catalogue

Input types: `image` = JPG/JPEG, PNG, WebP, BMP, GIF (first frame), TIFF; `pdf` = PDF.

### v1 (MVP)
| id | Label | Input | Output | Options (defaults **bold**) |
|---|---|---|---|---|
| `images_to_pdf` | Combine into PDF | 1+ images | 1 PDF | Page size: **Fit to image** (image DPI, else 150 DPI) / A4 / Letter (landscape for wide images) · Margin: **None** / Small (10 mm) · Order: **selection order** (by filename if unknown). JPEGs embedded without re-encoding; EXIF rotation applied. |
| `pdf_to_images` | PDF to JPG/PNG | 1 PDF | 1 image per page | **PdfPagesEditor**. Format: **JPG** / PNG · Size: Screen / **Standard** / Print (72 / 150 / 300 DPI) · Pages: **All** / picked pages or range "1-3,5" |
| `image_convert` | Convert to… | 1+ images | same count | Format: **JPG** / PNG / WebP (lossy, libwebp) · Quality 1–100 (**85**) for JPG/WebP · Transparent background → **white** (JPG only) |
| `image_resize` | Resize | 1+ images | same count | **ResizeEditor** (visual). Starts at long edge **1600 px** (or the original if smaller); never upscales. Several images: aspect locked → same percentage for each (each keeps its shape); unlocked → every image becomes exactly W × H. |
| `image_compress` | Compress | 1+ images | same count | Mode: **Target size (KB)** / Quality slider · Target default **200 KB** |
| `pdf_merge` | Merge PDFs | 2+ PDFs | 1 PDF | Order: **selection order** |
| `apply_preset` | Apply preset… | 1+ images | same count | Pick from presets (§8) |

### Phase 2
`pdf_split` (every page / ranges) · `pdf_compress` (re-encode embedded images at lower quality/DPI) · `pdf_rotate` · `image_crop` (CropEditor) · `image_rotate` · `heic_to_jpg` (libheif, LGPL, dynamic) · `strip_metadata` (remove EXIF/GPS) · PageTray for `images_to_pdf`.

### Phase 3
`video_to_gif`, `extract_audio`, `video_compress` (FFmpeg LGPL build as sidecar) · `ocr_to_text` (Tesseract, Apache-2.0) · `batch_rename`.

### Suggestion logic
- All images (1) → Suggested: Convert to…, Compress, Resize, Combine into PDF.
- All images (2+) → Suggested: Combine into PDF, Compress, Resize.
- 1 PDF → Suggested: PDF to JPG/PNG, (Phase 2: Compress PDF, Split).
- 2+ PDFs → Suggested: Merge PDFs.
- All other actions are still listed under their group, disabled with a reason when they don't fit.
- Mixed images + PDFs → show only actions valid for every file; if none, show the message "These files can't be converted together" plus per-type groups, then a *Not available* group.
- Unsupported types → listed in the chip as "2 files not supported" and excluded.

## 7. Compress-to-target-size algorithm (images)

Input: image, target bytes `T`, optional exact dimensions (from preset).
1. If preset has dimensions, resize first (Lanczos3), cropping centre-weighted to the aspect if needed.
2. Binary search JPEG quality q in [10, 95] (max 8 iterations) for the largest q with size ≤ `T`.
3. If even q = 10 is > `T`, downscale by 0.85 and repeat step 2 (max 6 rounds; never go below preset min dimensions).
4. If a preset has a **minimum** size (e.g. 20–50 KB) and output is below it, increase quality toward 95.
5. Report final size, dimensions and quality in the Result view (e.g. "48 KB · 200×230 · quality 78").
6. If the target can't be reached, fail that file with: "Couldn't get this below 50 KB without going under the minimum size."

## 8. Presets

Stored in `resources/presets.json` (shipped) and merged with user presets in the app config dir. Schema:

```jsonc
{
  "id": "exam_photo_example",
  "label": "Exam photo (example)",
  "format": "jpg",
  "width": 200, "height": 230,       // optional, px
  "aspect": "3.5:4.5",               // optional, used if width/height absent
  "longEdge": 1600,                  // optional, resize so the longer side is this many px
  "minKB": 20, "maxKB": 50,          // optional
  "dpi": 200,                        // optional, written to metadata
  "note": "Example values — always check the portal's official notice."
}
```

Shipped presets are clearly labelled **examples**; users can add, edit, duplicate and delete presets in Settings. Do not claim any preset matches an official specification.

## 9. Output rules

- Default location: **same folder as the original**. Setting: custom output folder.
- Naming:
  - Single conversion: `photo.png` → `photo.jpg`; if that exists → `photo (1).jpg`, `photo (2).jpg`…
  - Same-format operations: `photo.jpg` → `photo-compressed.jpg`, `photo-resized.jpg`, `photo-cropped.jpg`, `photo-<preset id>.jpg`.
  - `images_to_pdf`: `<first file name>.pdf` for one image; `<first file name> +N.pdf` for many.
  - `pdf_to_images`: folder `<pdf name> pages/` containing `<pdf name>-01.jpg`, `-02`, … (zero-padded to page count width).
  - `pdf_merge`: `<first file name> merged.pdf`.
- Originals are never touched. Writes go to a temp file in the target folder and are renamed on success (no half-written outputs).
- Colour profiles are kept: every image output (and the PDF image colour space) embeds the input's ICC profile, so wide-gamut phone photos (Display P3) don't look washed out.
- If the target folder isn't writable, fall back to the user's Downloads folder and say so in the Result view.

## 10. Settings window

Small separate window (480 × 560), opened from tray or Cmd/Ctrl+,.
- **Hotkey** — click-to-record field, conflict warning, Reset to default.
- **Output** — Next to original / Custom folder (picker).
- **Defaults** — JPEG quality (85), resize long edge (1600), PDF→image DPI (150).
- **Presets** — list with add / edit / duplicate / delete.
- **General** — Launch at login (`tauri-plugin-autostart`), Appearance: System / Light / Dark, Auto-hide after success (on).
- Settings persist as JSON in the app config directory.

## 11. Supported formats (v1)

Read: JPG/JPEG, PNG, WebP, BMP, GIF (first frame), TIFF, PDF.
Write: JPG, PNG, WebP, PDF.
HEIC/HEIF in Phase 2.

## 12. Non-functional requirements

| Area | Requirement |
|---|---|
| Speed | Popup visible ≤ 150 ms after hotkey. 20 phone JPGs → PDF ≤ 3 s on a mid-range laptop (JPEGs embedded without re-encoding). |
| Memory | Idle ≤ 100 MB RAM. Large files processed one at a time. |
| Privacy | No network access at all. No telemetry. |
| Reliability | Batches continue past individual failures and report per-file results. Jobs are cancellable. Crash-free on corrupt input (return an error). |
| Accessibility | Full keyboard use, visible focus, WCAG AA contrast in light and dark, reduced-motion respected. |
| Platforms | Windows 10/11 (x64), macOS 12+ (Apple Silicon + Intel). |
| Size | Installer target ≤ 25 MB for v1 (excluding FFmpeg in Phase 3). |

## 13. Error copy (examples)

- Unsupported: "QuickConvert can't open .xyz files yet."
- Corrupt: "This file looks damaged and couldn't be read."
- Password PDF: "This PDF is password-protected."
- Page range: "This PDF has only 10 pages." / "Write pages like 1-3, 5."
- Missing PDF engine: "PDF support is missing from this copy of QuickConvert. Please reinstall it."
- Disk/permission: "Couldn't save in this folder, so the file was saved to Downloads."
- Hotkey conflict: "Ctrl+Alt+Space is already used by another app. Pick a different shortcut."

## 14. Acceptance criteria for v1

- [ ] Selecting 5 JPGs in Explorer/Finder and pressing the hotkey shows the popup at the cursor with "Combine into PDF" as the first suggestion.
- [ ] Enter on that action produces a 5-page PDF next to the originals in ≤ 1 s for typical phone photos.
- [ ] A 10-page PDF → JPG at 150 DPI produces 10 correctly ordered files in a `… pages` folder.
- [ ] Compress to 50 KB on a 4 MB photo yields a file ≤ 50 KB, and reports size/quality.
- [ ] Originals are byte-identical before and after every action (tested).
- [ ] Popup never appears off-screen on any monitor, at any DPI.
- [ ] Every v1 action works using only the keyboard.
- [ ] Light and dark modes match DESIGN.md.
