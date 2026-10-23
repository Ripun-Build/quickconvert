# QuickConvert — Milestones

Work top to bottom. One milestone per commit (`M<n>: summary`). Each milestone must pass `cargo test`, `cargo clippy -- -D warnings` and `npx tsc --noEmit`.

> Host OS for development: **macOS**. Implement selection for the host OS first; the other OS gets a compiling stub.

## M0 — Scaffold
- [x] `npm create tauri-app@latest` (React + TS + Vite) in this repo root; keep `docs/`
- [x] Add Tailwind, Framer Motion, lucide-react; TS `strict: true`
- [x] Create `src/styles/tokens.css` from DESIGN.md §1 and map Tailwind theme to the variables
- [x] Create the project folder layout with empty modules that compile
- [x] `AppError` in `errors.rs` (serialises to `{ code, message }`)
- [x] `.gitignore` covers `node_modules`, `dist`, `src-tauri/target`
- [x] App runs with `npm run tauri dev`

## M1 — Tray + hotkey + popup shell
- [x] Tray icon with menu: Open QuickConvert · Settings… · Quit
- [x] No Dock icon on macOS (`ActivationPolicy::Accessory`); skip taskbar on Windows
- [x] Popup window created once at startup: transparent, borderless, always-on-top, hidden
- [x] Native blur via `window-vibrancy` with solid fallback (`data-blur="off"`)
- [x] Global hotkey (defaults per SPECS §4) shows the popup at the cursor
- [x] `popup/position.rs`: cursor monitor detection, flip near edges, clamp to work area with 8 px margin, HiDPI-correct; unit tests for the maths
- [x] Appear/hide animations (DESIGN §5); Esc and focus-loss hide
- [x] Measure: popup visible ≤ 150 ms after hotkey (log timing in dev)

## M2 — Getting files in
- [x] `get_selected_files()` for host OS (Windows COM on STA thread / macOS AppleScript)
- [x] Graceful fallback: no selection / permission denied → DropZone with hint
- [x] Drag-and-drop onto popup and "Choose files…" dialog
- [x] `thumbnail(path)` command (small PNG data URL, cached by path+mtime)
- [x] FileChip (single + multi fan), unsupported-file count
- [x] `lib/actions.ts` registry + suggestion logic (SPECS §6) with unit tests
- [x] ActionList view: search filter, groups, ↑/↓/Enter/1–9 keyboard, shared highlight

## M3 — Job runner + first conversions
- [x] `jobs.rs`: spawn_blocking, `job://progress` events, cancel flag, per-file results
- [x] `output.rs`: naming rules (SPECS §9), collision-safe names, temp-then-rename, Downloads fallback; tests
- [x] `images_to_pdf` (embed JPEGs without re-encoding; others → JPEG/PNG stream), page size + margin options
- [x] `pdf_merge` (lopdf)
- [x] `image_convert` (JPG/PNG/WebP, quality, transparency → white for JPG)
- [x] `image_resize` (long edge / exact / percent, Lanczos3) + visual ResizeEditor (drag handles, live W × H)
- [x] OptionsSheet, Running, Result and ErrorView views
- [x] Test: originals byte-identical after every action
- [x] Colour profiles (ICC) kept in JPG/PNG/WebP outputs and PDF images

## M4 — PDF to images
- [x] Bundle pdfium dynamic library as a resource; load at runtime with clear error if missing
- [x] `pdf_to_images` with format, DPI and page-range parsing (tests for "1-3,5")
- [x] Output folder `<name> pages/` with zero-padded names
- [x] Password-protected PDF → friendly error
- [x] `scripts/fetch-pdfium.sh` (pinned version + SHA-256; binary git-ignored, licences kept)
- [x] PdfPagesEditor: page thumbnails, click/Shift-click selection synced with the range field, Screen/Standard/Print sizes
- [x] Action list shows every action; unavailable ones disabled with a reason

## M5 — Compress + presets
- [x] `image_compress` quality mode
- [x] Target-size algorithm (SPECS §7) with tests on fixtures (hits ≤ target, respects min)
- [x] Load `resources/presets.json` + user presets; `apply_preset` action
- [x] Result view shows final size · dimensions · quality
- [x] Compress keeps the format (JPG/WebP by quality, PNG lossless + pixel reduction); live size estimate on the first image
- [x] Preset picker with plain-language summaries; KB limits pass under both 1000- and 1024-byte readings

## M6 — Settings window
- [x] Separate 480×560 window, opened from tray / Cmd/Ctrl+,
- [x] Hotkey recorder with conflict detection + reset
- [x] Output location, defaults, appearance (System/Light/Dark), auto-hide toggle
- [x] Preset editor (add / edit / duplicate / delete)
- [x] Launch at login (`tauri-plugin-autostart`)
- [x] Settings persisted as JSON in app config dir
- [x] Shortcut conflict at startup opens Settings → General with the message; system shortcuts refused
- [x] Output folder, defaults and auto-hide applied across the popup and jobs

## M7 — Polish + v1 release
- [x] Walk through every acceptance criterion in SPECS §14 and fix gaps (automated tests + manual check)
- [x] Reduced-motion and keyboard-only pass; contrast check in both themes (text tokens now WCAG AA)
- [x] Batch of 50 files: progress, cancel, per-file failure report
- [x] App icon + tray icons (light/dark template on macOS)
- [x] `npm run tauri build` produces installer ≤ 25 MB (universal DMG: 14.3 MB)
- [x] README with screenshots, install steps, and licences of bundled libraries

## Phase 2 — the v0.2.0 update
v0.1.0 is public, so Phase 2 ships as an update of the same app: settings and presets saved by v0.1.0 keep working, and one v0.2.0 release goes out when all of it is done.

### M8 — Crop and rotate
- [x] CropEditor (shapes incl. 35×45 mm passport, rotate) + `image_crop`
- [x] Rotate as its own action + `image_rotate`: "make every image Straight / Left / Right / Upside down", each image checked for which way is up (Vision: text + faces), per-image turns, click to correct, already-right images skipped

### M8b — Popup handling (feedback on M8)
- [x] Close only with the × button or ⌘W (no Esc / click-away closing)
- [x] Drag the popup by its header; it stays put when its size changes
- [x] One copy at a time: a second copy explains and quits
- [x] Closing Settings brings the popup back on the screen it was showing

### M9 — HEIC and metadata
- [x] HEIC/HEIF photos as input for every image action (macOS built-in `sips`); same-format outputs become a JPG sized like the original (quality 90/85/80)
- [x] `strip_metadata` ("Remove location & camera info"): JPG/PNG/WebP cleaned without touching the picture, HEIC stays HEIC, orientation kept, `-clean` names, result says what was removed
- [x] Combine into PDF: embedded JPEGs lose their metadata; HEIC pages stored as JPEG
- [x] Speed: batches and PDF pages 4 at a time, HEIC thumbnails/checks from small `sips` copies, fewer JPG encodes (50 HEIC → PDF: 22 s → 4.5 s)

### M10 — PDF tools
- [x] Split PDF (`pdf_split`): click where parts start or Every page / 2 / 3, parts into a `<name> parts` folder; Keep pages (`pdf_extract`) → one new PDF
- [x] Rotate pages (`pdf_rotate`): page grid with the same Straight / Left / Right / Upside down detection as images; lossless `/Rotate`
- [x] Make PDF smaller (`pdf_compress`): Smallest / Balanced / Print with a live size, pictures scaled to the page and re-saved, extras removed; "already small" reported, not failed

### M11 — PageTray
- [ ] PageTray (reorder/rotate/remove before PDF)

### M12 — v0.2.0 release
- [ ] Version 0.2.0, release notes, README (use + screenshots) for the new behaviour, v0.1.0 settings/presets still load, universal DMG, GitHub release

## Later updates
- [ ] Other OS: implement real selection, test, package (its own update, once it can be tested on Windows)

## Phase 3
- [ ] Explorer/Finder right-click entry
- [ ] FFmpeg sidecar: video → GIF, extract audio, compress video
- [ ] OCR (Tesseract), batch rename
- [ ] Code signing / notarisation, auto-update (opt-in, the only allowed network call)
