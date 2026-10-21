#!/usr/bin/env python3
"""Write THIRD_PARTY_LICENSES.md: every Rust crate and npm package shipped in the app, plus pdfium.

Run from the repo root after `npm install`:  python3 scripts/third-party-licenses.py
"""
import json
import pathlib
import subprocess

ROOT = pathlib.Path(__file__).resolve().parent.parent
OUT = ROOT / "THIRD_PARTY_LICENSES.md"

# Components inside the pdfium library (src-tauri/resources/pdfium/licenses/ has the full texts).
PDFIUM_PARTS = [
    ("PDFium", "BSD-3-Clause", "pdfium.txt"),
    ("pdfium-binaries build (bblanchon)", "MIT", "../LICENSE-pdfium-binaries.txt"),
    ("Abseil", "Apache-2.0", "abseil.txt"),
    ("Anti-Grain Geometry 2.3", "AGG (permissive)", "agg23.txt"),
    ("dragonbox", "Apache-2.0 WITH LLVM-exception / BSL-1.0", "dragonbox-Apache2-LLVM.txt"),
    ("fast_float", "MIT", "fast_float.txt"),
    ("FreeType", "FTL (FreeType License)", "freetype.txt"),
    ("HarfBuzz", "MIT", "harfbuzz.txt"),
    ("Highway", "Apache-2.0", "highway.txt"),
    ("ICU", "Unicode-3.0", "icu.txt"),
    ("Little CMS", "MIT", "lcms.txt"),
    ("libjpeg-turbo", "IJG / BSD-3-Clause / Zlib", "libjpeg_turbo.md"),
    ("OpenJPEG", "BSD-2-Clause", "libopenjpeg.txt"),
    ("libpng", "libpng-2.0", "libpng.txt"),
    ("LLVM libc", "Apache-2.0 WITH LLVM-exception", "llvm-libc.txt"),
    ("simdutf", "MIT / Apache-2.0", "simdutf.txt"),
    ("zlib", "Zlib", "zlib.txt"),
]


def rust_crates():
    """Normal (shipped) dependencies for the macOS build, name → (version, licence)."""
    crates = {}
    for target in ("aarch64-apple-darwin", "x86_64-apple-darwin"):
        out = subprocess.run(
            ["cargo", "tree", "-e", "normal", "--target", target, "--prefix", "none", "--format", "{p}\t{l}"],
            cwd=ROOT / "src-tauri",
            capture_output=True,
            text=True,
            check=True,
        ).stdout
        for line in out.splitlines():
            if "\t" not in line:
                continue
            pkg, licence = line.split("\t", 1)
            parts = pkg.split()
            name, version = parts[0], parts[1].lstrip("v") if len(parts) > 1 else ""
            if name == "quickconvert":
                continue
            crates[(name, version)] = licence.strip() or "see crate"
    return sorted(crates.items())


def npm_packages():
    """Production npm packages (bundled into the UI), name → (version, licence)."""
    tree = json.loads(
        subprocess.run(
            ["npm", "ls", "--omit=dev", "--all", "--json"], cwd=ROOT, capture_output=True, text=True
        ).stdout
    )
    found = {}

    def walk(deps):
        for name, info in (deps or {}).items():
            version = info.get("version", "")
            if (name, version) in found:
                continue
            manifest = ROOT / "node_modules" / name / "package.json"
            licence = "see package"
            if manifest.exists():
                data = json.loads(manifest.read_text())
                licence = data.get("license") or licence
                if isinstance(licence, dict):
                    licence = licence.get("type", "see package")
            found[(name, version)] = licence
            walk(info.get("dependencies"))

    walk(tree.get("dependencies"))
    return sorted(found.items())


def table(rows):
    lines = ["| Package | Version | Licence |", "|---|---|---|"]
    lines += [f"| {name} | {version} | {licence} |" for (name, version), licence in rows]
    return "\n".join(lines)


def main():
    rust = rust_crates()
    npm = npm_packages()
    pdfium = "\n".join(
        ["| Component | Licence | Full text |", "|---|---|---|"]
        + [
            f"| {name} | {licence} | [{file.split('/')[-1]}](src-tauri/resources/pdfium/licenses/{file}) |"
            for name, licence, file in PDFIUM_PARTS
        ]
    )
    OUT.write_text(
        f"""# Third-party licences

QuickConvert is built on open-source software. Everything it ships is under a permissive licence
(MIT, Apache-2.0, BSD, ISC, Zlib or similar); nothing is GPL. Regenerate this file with
`python3 scripts/third-party-licenses.py`.

## PDF engine (pdfium, bundled as a library)

Pre-built by [bblanchon/pdfium-binaries](https://github.com/bblanchon/pdfium-binaries) (version pinned in
`scripts/fetch-pdfium.sh`). Full licence texts ship inside the app next to the library.

{pdfium}

The FreeType project is used under the FreeType License (FTL): portions of this software are
copyright © The FreeType Project (www.freetype.org). All rights reserved.

## Rust crates ({len(rust)})

{table(rust)}

## JavaScript packages ({len(npm)})

{table(npm)}
"""
    )
    print(f"wrote {OUT.relative_to(ROOT)}: {len(rust)} crates, {len(npm)} npm packages")


if __name__ == "__main__":
    main()
