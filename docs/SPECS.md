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
7. Esc goes back one step. The popup stays open until the user closes it with its **×** button (or ⌘W / Ctrl+W); clicking elsewhere doesn't close it.

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
- **Moving:** drag the header (anywhere but its buttons) to move the popup with the mouse or trackpad. Until the next show it stays where it was left: size changes (e.g. opening an editor) keep its top-left, nudged only as far as needed to stay on screen. The next hotkey press places it at the cursor again.

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
| **CompressView** | Target size (chips 20 / 50 / 100 / 200 / 500 KB / 1 MB + "Other" KB field) or Quality (slider; PNG explains it has no quality). A live card shows the real result for the first image: "4.1 MB → 48 KB · 1600×1200 · quality 78". |
| **PresetView** | Preset cards (label + what it does, e.g. "200 × 230 px · 20–50 KB · JPG · 200 DPI"), the preset's note, and the same live result card. |
| **CropEditor** | Popup widens to 720 px. The photo with a crop box on top; everything outside the box is dimmed. Drag inside the box to move it; 4 corner + 4 side handles resize it (rule-of-thirds grid while dragging, live W × H label). Shapes: **Free** / Square / 4:3 / 3:4 / 16:9 / Passport 35×45 — picking one fits the largest centred box of that shape, and handles then keep it (a corner follows the axis the pointer moved further along). Passport shows whether the crop is big enough to print sharply at 35 × 45 mm (≥ 413 × 531 px). Width/Height fields (the box keeps its centre). Rotate left/right turns the photo (spring; the box re-fits to the shape). Reset. Keyboard: arrows move the box, arrows on the corner handle resize it (Shift = 10×). |
| **RotateView** | "Make every image" **Straight** / Left / Right / Upside down (Segmented). Below, every selected image as it will come out (one image: a 176 px picture; several: a 4-column grid that scrolls), turning with a spring as things change. Each image is checked for which way is up (§6 `image_rotate`); a badge says what happens to it: ↻ will be turned, ✓ already right (dimmed, skipped), ? couldn't tell (left as it is). Click a picture to turn it a quarter turn right, Shift-click or ← to turn it left (→ also turns right). One line sums it up ("2 will be turned · 1 already right, skipped · 1 couldn't tell (click to turn)"; "Checking which way is up… 3 of 5" while checking). Button "Rotate 2 of 4 images" / "Rotate" / "Nothing to turn". |
| **SplitEditor** | Popup widens to 720 px. Page grid (the PDF → images thumbnails) on the left; right column: Segmented **Split into parts** / Keep pages. Split: each tile is captioned "4 · Part 2", pages that start a part get an accent ring and a ✂ badge; quick chips Every page / Every 2 / Every 3 / Clear; the parts listed ("Part 2 · pages 4–7", first six then "+ N more"). Keep: picked pages ringed with a ✓, All / None, a "2, 5-7" field in sync both ways. Button "Split into 3 PDFs" / "Save 2 pages as a new PDF". |
| **PageRotateView** | Popup widens to 720 px. "Make every page" Segmented over a 4-column grid of square cells, so a page can turn inside its cell; same badges, clicks and summary line as RotateView. Button "Rotate 3 pages" / "Nothing to turn" ("Checking…" until every page is checked). |
| **PdfCompressView** | Three radio cards (Smallest / Balanced / Print, each with a one-line use) and a live card "24 MB → 3.1 MB" for the first PDF ("Already about as small as it gets: no copy will be saved." when under 5 %). Button "Make smaller · 3 PDFs". |
| **PageTray** | Phase 2. Thumbnails of images/pages; drag to reorder, rotate, remove; then "Create PDF". |

### Keyboard
- Typing anywhere focuses the search field and filters actions (fuzzy match on label + keywords).
- ↑/↓ move highlight, Enter runs/opens options, 1–9 pick the Nth visible action (while the search field is empty).
- Esc: clears the search, goes back one view, or cancels a running job; it never closes the popup. Cmd/Ctrl+W closes it. Cmd/Ctrl+, opens Settings.
- Visible focus ring on all controls.

### Dismissal
- Hide only on: the **×** button in the header (title "Close (⌘W) · QuickConvert stays in the menu bar"), Cmd/Ctrl+W, or the successful-result auto-hide (Settings → auto-hide).
- Clicking outside or switching apps never hides it, so files can always be dragged in from Explorer/Finder and a running job stays in view.
- Opening Settings sets the popup aside (it floats above other windows); closing Settings brings it back exactly as it was: same screen, files and position.

### One copy at a time
- Only one QuickConvert runs at once (an OS file lock in the app config folder, released automatically when the app exits or crashes). Two copies would both claim the hotkey and it would seem to do nothing.
- A second copy registers nothing, shows "QuickConvert is already running — use it from its menu bar icon or its keyboard shortcut, or quit it first and then open this one again", and quits when the message is closed.

## 6. Action catalogue

Input types: `image` = JPG/JPEG, PNG, WebP, BMP, GIF (first frame), TIFF, HEIC/HEIF; `pdf` = PDF.

**HEIC/HEIF (iPhone photos, v0.2):** read on macOS through the built-in `/usr/bin/sips` into a temporary TIFF in the system temp folder (pixels, colour profile and orientation tag kept; deleted right after), then loaded like any image — so every image action, thumbnail and editor accepts HEIC. Operations that keep the format (resize, crop, rotate) save HEIC as **JPG**, sized like the original: HEIC packs a photo into about half the space of a JPG, so the JPG uses the highest quality of 90 / 85 / 80 whose file isn't bigger than the HEIC would be at that pixel count (floor 80). Compress and presets follow their own targets; Remove info keeps HEIC (below). On other systems HEIC is "can't open .heic files yet". The Mac-only part lives in `heic/` (like `selection/` and `upright/`).

### v1 (MVP)
| id | Label | Input | Output | Options (defaults **bold**) |
|---|---|---|---|---|
| `images_to_pdf` | Combine into PDF | 1+ images | 1 PDF | Page size: **Fit to image** (image DPI, else 150 DPI) / A4 / Letter (landscape for wide images) · Margin: **None** / Small (10 mm) · Order: **selection order** (by filename if unknown). JPEGs embedded without re-encoding, minus their metadata blocks (location, camera, date, XMP, comments; v0.2); EXIF rotation applied. HEIC pages stored as JPEG sized like the original (quality 90 / 85 / 80, as above) to keep the PDF small. |
| `pdf_to_images` | PDF to JPG/PNG | 1 PDF | 1 image per page | **PdfPagesEditor**. Format: **JPG** / PNG · Size: Screen / **Standard** / Print (72 / 150 / 300 DPI) · Pages: **All** / picked pages or range "1-3,5" |
| `image_convert` | Convert to… | 1+ images | same count | Format: **JPG** / PNG / WebP (lossy, libwebp) · Quality 1–100 (**85**) for JPG/WebP · Transparent background → **white** (JPG only) |
| `image_resize` | Resize | 1+ images | same count | **ResizeEditor** (visual). Starts at long edge **1600 px** (or the original if smaller); never upscales. Several images: aspect locked → same percentage for each (each keeps its shape); unlocked → every image becomes exactly W × H. |
| `image_compress` | Compress | 1+ images | same count | **CompressView**. Mode: **Target size (KB)** / Quality slider (default 75) · Target default **200 KB**. Keeps the format: JPG → JPG, WebP → WebP (quality), PNG → PNG (strongest lossless compression; size targets reduce pixels); BMP / TIFF / GIF → PNG; HEIC → JPG. Colour profile kept, EXIF/GPS dropped. |
| `pdf_merge` | Merge PDFs | 2+ PDFs | 1 PDF | Order: **selection order** |
| `apply_preset` | Apply preset… | 1+ images | same count | **PresetView**. Pick from presets (§8); output in the preset's format, named `photo-<preset id>.jpg`. |

### v0.2 (Phase 2)
| id | Label | Input | Output | Options (defaults **bold**) |
|---|---|---|---|---|
| `image_crop` | Crop | 1 image | 1 image | **CropEditor** (visual). Box in pixels of the upright photo after an optional quarter turn; shape **Free**. Keeps the format (JPG/WebP saved at quality 90) and colour profile. Several images selected: listed but disabled ("Works on one image at a time"). |
| `image_rotate` | Rotate | 1+ images | only the images that need turning | **RotateView**. The user picks how every image should end up: **Straight** / Left / Right / Upside down. Each image is first checked for which way is up (after its EXIF orientation is applied): on macOS the system Vision framework (offline, via `osascript`; nothing bundled) reads it four ways — text scored by letters of real words (upside-down text reads as gibberish), faces by confidence × how level they are — and a reading wins when it clearly beats the others (text ≥ 12 letters, faces ≥ 0.6, at least 2× the runner-up). No clear winner (landscapes, little text): "couldn't tell", treated as already straight. Each image's turn = what makes it straight + the target + the user's own clicks; images whose turn comes to 0 are skipped, not saved again. Keeps the format (JPG/WebP saved at quality 90) and colour profile; the result is upright pixels, not an orientation tag (many upload sites ignore the tag). Other systems: no detection yet (every image "couldn't tell"). |

| `strip_metadata` | Remove location & camera info | 1+ images | same count | No options: runs straight away. Removes location (GPS), camera/phone and lens details, dates, editing software, descriptions, XMP/IPTC blocks and comments, plus anything stored after the end of a JPEG (phones keep depth/HDR maps there, each with its own copy of the metadata). **JPG, PNG, WebP: the picture data is copied byte-for-byte** — only metadata blocks are dropped; kept: colour profile, JFIF/print resolution, PNG colour/transparency chunks, WebP animation. A "turn to view" orientation survives as a minimal EXIF block with nothing else in it, so sideways photos still show upright. TIFF and BMP are re-saved (lossless; TIFF's colour profile isn't kept), GIF keeps its first frame. **HEIC stays HEIC**: the clean pixels (upright, colour profile only) are re-encoded once by the system encoder at its normal camera quality (`sips -s format heic`), so the copy is about the original's size. Output `photo-clean.jpg` / `photo-clean.heic`. The Result view says what was found: "Removed location, camera and date info and other details." / "(3 of 5 images had some)" / "No location or camera info found. Saved a clean copy anyway." |
| `pdf_split` | Split PDF | 1 PDF | 2+ PDFs in a folder, or 1 PDF | **SplitEditor**. Two modes. **Split into parts**: click the pages where new parts start (page 1 always starts part 1), or Every page / Every 2 / Every 3 / Clear; written to a new folder `<name> parts/` as `<name> (pages 1-3).pdf`, `<name> (page 4).pdf`… (parts built 4 at a time, written in order; the folder is removed if nothing got written). **Keep pages** (`pdf_extract`): pick pages (click, Shift-click a range, or type "2, 5-7") → one PDF next to the original, `<name> (pages 2, 5-7).pdf` (a list longer than 30 characters becomes "(12 pages)"). Pages are copied as they are; bookmarks and form structure stay behind, and links to pages left behind are removed so they don't drag those pages along. |
| `pdf_rotate` | Rotate pages | 1 PDF | 1 PDF | **PageRotateView**: image Rotate for pages. "Make every page **Straight** / Left / Right / Upside down"; each page is rendered small (as it shows now, rotation included) and checked for which way is up with the same Vision check as images; click a page to turn it right, Shift-click / ← left. Only each page's `/Rotate` changes (added to what it had, inherited values included); content untouched. Output `<name>-rotated.pdf`. Nothing to turn → plain-language message. |
| `pdf_compress` | Make PDF smaller | 1+ PDFs | same count, or none | **PdfCompressView**: **Smallest** (pictures up to 100 pixels per inch, JPEG quality 60) / **Balanced** (150, 75; default) / **Print** (220, 85), with the real result size of the first PDF measured live. Pictures drawn on pages (also inside form XObjects) are scaled to fit the largest page they appear on at that resolution and re-saved as JPEG — only 8-bit gray/RGB (incl. ICC-based) pictures stored as JPEG, Flate/LZW or raw; masks, colour-key masks, Decode arrays, CMYK/indexed/16-bit, JPEG 2000, JBIG2 and fax pictures are left as they are, and a picture is only replaced when it gets ≥ 10 % smaller. Picture data is unpacked with a size limit, so a "zip bomb" can't exhaust memory. XMP metadata, app-private data and page thumbnails are removed, unused objects pruned, streams compressed, and the file saved with object streams. Text, fonts and drawings are untouched. Under 5 % smaller → no copy; the Result says "Already about as small as it gets". Output `<name>-compressed.pdf`. |

`heic_to_jpg` is covered by Convert to… now that HEIC is a normal input (typing "heic" finds it).

Still to come in v0.2: PageTray for `images_to_pdf`.

PDFs that open without a password are saved without their encryption by these edits; password-protected PDFs get the usual "This PDF is password-protected." Editing a signed PDF invalidates its signature (as any edit does).

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
3. If even q = 10 is > `T`, shrink and repeat step 2: first jump to ≈ √(T / size at q 10) × 0.95 (file size grows with pixel count), then up to 6 more rounds of × 0.85. Never below 64 px on the long edge; presets with exact width × height never shrink. PNG has no quality, so only steps 3–6 apply.
4. If a preset has a **minimum** size (e.g. 20–50 KB) and output is below it, increase quality toward 95.
5. Report final size, dimensions and quality in the Result view (e.g. "48 KB · 200×230 · quality 78").
   - "KB" is ambiguous (1000 or 1024 bytes), so maximums are met as `max × 1000` bytes and minimums as `min × 1024`: the file passes either reading.
6. If the target can't be reached, fail that file with: "Couldn't get this below 50 KB without going under the minimum size."

## 8. Presets

Stored in `resources/presets.json` (shipped) and merged with user presets in `presets.json` in the app config dir (`{ "presets": [...], "removed": [ids] }`: a user preset with the same `id` replaces the shipped one, `removed` hides deleted built-ins, only differences from the built-ins are stored; an unreadable file is ignored). Width + height resize and centre-crop to fill; `aspect` centre-crops; `longEdge` never upscales. Schema:

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
  - Same-format operations: `photo.jpg` → `photo-compressed.jpg`, `photo-resized.jpg`, `photo-cropped.jpg`, `photo-rotated.jpg`, `photo-clean.jpg`, `photo-<preset id>.jpg`. HEIC inputs get `.jpg` names, except Remove info (`photo-clean.heic`).
  - `images_to_pdf`: `<first file name>.pdf` for one image; `<first file name> +N.pdf` for many.
  - `pdf_to_images`: folder `<pdf name> pages/` containing `<pdf name>-01.jpg`, `-02`, … (zero-padded to page count width).
  - `pdf_merge`: `<first file name> merged.pdf`.
  - `pdf_split`: folder `<pdf name> parts/` with `<pdf name> (pages 1-3).pdf`, `<pdf name> (page 4).pdf`…; Keep pages: `<pdf name> (pages 2, 5-7).pdf` (or `(12 pages)` for a long list).
  - `pdf_rotate`: `<pdf name>-rotated.pdf` · `pdf_compress`: `<pdf name>-compressed.pdf`.
- Originals are never touched. Writes go to a temp file in the target folder and are renamed on success (no half-written outputs).
- Colour profiles are kept: every image output (and the PDF image colour space) embeds the input's ICC profile, so wide-gamut phone photos (Display P3) don't look washed out.
- If the target folder isn't writable, fall back to the user's Downloads folder and say so in the Result view.

## 10. Settings window

Separate 480 × 560 window with toolbar tabs **General · Conversions · Presets**, opened from the tray (Settings…), the popup's gear button, or Cmd/Ctrl+, in the popup. It's created when first opened (never on the hotkey path) and every change applies immediately.

- **General**
  - **Shortcut** — click the field, press the new shortcut (Esc cancels); the current one is paused while recording. It must include ⌃/⌥/⌘ (Ctrl/Alt/Win). It's tested by registering it: if another app owns it, "⌃⌥K is already used by another app. Pick a different shortcut." and the old one stays. System shortcuts that would never fire (⌘Space, ⌃Space, ⌘Tab, ⌘⇧3/4/5, ⌘Q/W/H/M; Alt+Shift, Win+… on Windows) are refused with who uses them. Reset returns to the default.
  - **Appearance** — System / Light / Dark, applied to every window including the native blur.
  - **Launch at login** (`tauri-plugin-autostart`) and **Hide after saving** (on).
  - If the shortcut can't be registered at startup, Settings opens on General with the conflict message (plus the tray menu note).
- **Conversions**
  - **Save converted files** — Next to the original / In a folder (picker). The folder applies to every action and is recreated if it was removed; Downloads stays the fallback when it isn't writable.
  - **Defaults** — JPEG quality (85) for Convert, resize long edge (1600) for the Resize editor, PDF image size (Standard = 150 DPI) for PDF to JPG/PNG.
- **Presets** — list (built-ins marked *Example*) with Add / Duplicate / Delete and an editor: name, format, size (exact W × H / ratio / long edge / keep), at least / at most KB, DPI, note; saves as you type. *Restore built-in presets* brings back deleted or edited built-ins and keeps the user's own.
- Settings persist as `settings.json` in the app config directory (a missing or broken file means defaults; out-of-range values are corrected). Launch at login is owned by the OS login items, not this file.

## 11. Supported formats (v1)

Read: JPG/JPEG, PNG, WebP, BMP, GIF (first frame), TIFF, PDF.
Write: JPG, PNG, WebP, PDF.
HEIC/HEIF in Phase 2.

## 12. Non-functional requirements

| Area | Requirement |
|---|---|
| Speed | Popup visible ≤ 150 ms after hotkey. 20 phone JPGs → PDF ≤ 3 s on a mid-range laptop (JPEGs embedded without re-encoding). Batches and Combine into PDF work on up to 4 files at once (`pool.rs`; results and pages stay in selection order, progress never steps back, Cancel lets started files finish); 50 iPhone HEIC photos → PDF ≈ 5 s on an Apple-silicon Mac. HEIC thumbnails and Rotate checks ask `sips` for a small copy (`-Z`) instead of decoding the full photo. |
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
- Size target: "Couldn't get this below 20 KB without making it too small." / "Couldn't get this above 10 KB at 140×60."
- Missing PDF engine: "PDF support is missing from this copy of QuickConvert. Please reinstall it."
- Disk/permission: "Couldn't save in this folder, so the file was saved to Downloads."
- Hotkey conflict: "Ctrl+Alt+Space is already used by another app. Pick a different shortcut."

## 14. Acceptance criteria for v1

- [x] Selecting 5 JPGs in Explorer/Finder and pressing the hotkey shows the popup at the cursor with "Combine into PDF" as the first suggestion.
- [x] Enter on that action produces a 5-page PDF next to the originals in ≤ 1 s for typical phone photos.
- [x] A 10-page PDF → JPG at 150 DPI produces 10 correctly ordered files in a `… pages` folder.
- [x] Compress to 50 KB on a 4 MB photo yields a file ≤ 50 KB, and reports size/quality.
- [x] Originals are byte-identical before and after every action (tested).
- [x] Popup never appears off-screen on any monitor, at any DPI.
- [x] Every v1 action works using only the keyboard.
- [x] Light and dark modes match DESIGN.md.
