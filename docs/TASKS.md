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
- [ ] `image_compress` quality mode
- [ ] Target-size algorithm (SPECS §7) with tests on fixtures (hits ≤ target, respects min)
- [ ] Load `resources/presets.json` + user presets; `apply_preset` action
- [ ] Result view shows final size · dimensions · quality

## M6 — Settings window
- [ ] Separate 480×560 window, opened from tray / Cmd/Ctrl+,
- [ ] Hotkey recorder with conflict detection + reset
- [ ] Output location, defaults, appearance (System/Light/Dark), auto-hide toggle
- [ ] Preset editor (add / edit / duplicate / delete)
- [ ] Launch at login (`tauri-plugin-autostart`)
- [ ] Settings persisted as JSON in app config dir

## M7 — Polish + v1 release
- [ ] Walk through every acceptance criterion in SPECS §14 and fix gaps
- [ ] Reduced-motion and keyboard-only pass; contrast check in both themes
- [ ] Batch of 50 files: progress, cancel, per-file failure report
- [ ] App icon + tray icons (light/dark template on macOS)
- [ ] `npm run tauri build` produces installer ≤ 25 MB
- [ ] README with screenshots, install steps, and licences of bundled libraries

## Phase 2 (after v1)
- [ ] CropEditor (aspect presets, rotate) + `image_crop`
- [ ] PageTray (reorder/rotate/remove before PDF)
- [ ] `pdf_split`, `pdf_compress`, `pdf_rotate`, `image_rotate`
- [ ] `heic_to_jpg` via libheif (dynamic, LGPL)
- [ ] `strip_metadata`
- [ ] Other OS: implement real selection, test, package

## Phase 3
- [ ] Explorer/Finder right-click entry
- [ ] FFmpeg sidecar: video → GIF, extract audio, compress video
- [ ] OCR (Tesseract), batch rename
- [ ] Code signing / notarisation, auto-update (opt-in, the only allowed network call)
