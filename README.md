# QuickConvert

Select files → press a hotkey → convert, compress or crop them from a small popup at your cursor. Offline, private, Apple-style.

> Status: in development.

## Docs
- [docs/SPECS.md](docs/SPECS.md) — product specification
- [docs/DESIGN.md](docs/DESIGN.md) — design system
- [docs/TASKS.md](docs/TASKS.md) — milestone checklist

## Prerequisites
- Rust (via rustup) and Node.js LTS
- **Windows:** Visual Studio C++ Build Tools ("Desktop development with C++"), WebView2 (preinstalled on Windows 11)
- **macOS:** Xcode Command Line Tools (`xcode-select --install`)

## Development

```bash
npm install
sh scripts/fetch-pdfium.sh # once: PDF engine (pinned version, checksum-verified)
npm run tauri dev          # run the app
npm run tauri build        # production bundle
```
