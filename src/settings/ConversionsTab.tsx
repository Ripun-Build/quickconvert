// Conversions: where files are saved, and the defaults that pre-fill the popup.
import { useEffect, useRef, useState } from "react";
import { FolderOpen } from "lucide-react";
import { Button } from "../components/Button";
import { Slider } from "../components/Fields";
import { Segmented } from "../components/Segmented";
import { chooseFolder } from "../lib/ipc";
import { SIZES } from "../lib/pages";
import type { Engine, Settings } from "../lib/types";
import { Group, Row } from "./ui";

const SAVE_DELAY_MS = 300;

/** Convert to PDF: the ways offered per Office file type ("auto" = the best way available). */
const OFFICE_ROWS: { key: "word" | "slides" | "sheet"; label: string; choices: { value: string; label: string }[] }[] = [
  {
    key: "word",
    label: "Word to PDF",
    choices: [
      { value: "auto", label: "Auto" },
      { value: "quick", label: "Quick" },
      { value: "pages", label: "Pages" },
      { value: "libreoffice", label: "LibreOffice" },
    ],
  },
  {
    key: "slides",
    label: "PowerPoint to PDF",
    choices: [
      { value: "auto", label: "Auto" },
      { value: "keynote", label: "Keynote" },
      { value: "libreoffice", label: "LibreOffice" },
    ],
  },
  {
    key: "sheet",
    label: "Excel to PDF",
    choices: [
      { value: "auto", label: "Auto" },
      { value: "quick", label: "Quick" },
      { value: "numbers", label: "Numbers" },
      { value: "libreoffice", label: "LibreOffice" },
    ],
  },
];

export function ConversionsTab({ settings, onChange }: { settings: Settings; onChange: (patch: Partial<Settings>) => void }) {
  const folder = settings.output.kind === "folder" ? settings.output.path : null;

  // Sliders and typing save after a short pause, not on every step.
  const [quality, setQuality] = useState(settings.jpegQuality);
  const [longEdge, setLongEdge] = useState(String(settings.resizeLongEdge));
  const timer = useRef<ReturnType<typeof setTimeout>>(undefined);
  const later = (patch: Partial<Settings>) => {
    clearTimeout(timer.current);
    timer.current = setTimeout(() => onChange(patch), SAVE_DELAY_MS);
  };
  useEffect(() => setQuality(settings.jpegQuality), [settings.jpegQuality]);
  useEffect(() => setLongEdge(String(settings.resizeLongEdge)), [settings.resizeLongEdge]);

  const pickFolder = () =>
    chooseFolder()
      .then((path) => path && onChange({ output: { kind: "folder", path } }))
      .catch(() => {});

  return (
    <>
      <Group title="Save converted files">
        <label className="flex min-h-11 cursor-default items-center gap-2.5 px-3 py-2">
          <input
            type="radio"
            name="output"
            checked={!folder}
            onChange={() => onChange({ output: { kind: "next_to_original" } })}
            className="accent-accent"
          />
          <span className="type-body text-primary">Next to the original</span>
        </label>
        <div className="flex min-h-11 items-center gap-2.5 px-3 py-2">
          <input
            type="radio"
            name="output"
            aria-label="In a folder"
            checked={Boolean(folder)}
            onChange={() => (folder ? undefined : void pickFolder())}
            className="accent-accent"
          />
          <div className="min-w-0 flex-1">
            <p className="type-body text-primary">In a folder</p>
            {folder && (
              <p className="type-caption truncate text-tertiary" title={folder}>
                {folder}
              </p>
            )}
          </div>
          <Button onClick={() => void pickFolder()}>
            <FolderOpen size={14} strokeWidth={1.75} />
            Choose…
          </Button>
        </div>
      </Group>

      <Group title="Defaults">
        <Row label="JPEG quality" description="Starting value for Convert to JPG / WebP.">
          <div className="w-[200px]">
            <Slider
              label="JPEG quality"
              value={quality}
              min={1}
              max={100}
              step={1}
              onChange={(v) => {
                setQuality(v);
                later({ jpegQuality: v });
              }}
            />
          </div>
        </Row>
        <Row label="Resize long edge" description="Starting size in the Resize editor.">
          <label className="flex h-[30px] w-[104px] items-center gap-1.5 rounded-control bg-surface-raised px-2 shadow-control focus-within:ring-[3px] focus-within:ring-focus-ring">
            <input
              inputMode="numeric"
              aria-label="Resize long edge in pixels"
              value={longEdge}
              onChange={(e) => {
                const digits = e.target.value.replace(/\D/g, "").slice(0, 5);
                setLongEdge(digits);
                if (Number(digits) >= 16) later({ resizeLongEdge: Number(digits) });
              }}
              onBlur={() => setLongEdge(String(settings.resizeLongEdge))}
              className="type-body min-w-0 flex-1 bg-transparent text-primary tabular-nums outline-none"
            />
            <span className="type-caption text-tertiary">px</span>
          </label>
        </Row>
        <Row label="PDF image size" description="Starting size for PDF to JPG/PNG.">
          <div className="w-[228px]">
            <Segmented
              label="PDF image size"
              value={String(settings.pdfDpi)}
              choices={SIZES.map((s) => ({ value: String(s.dpi), label: s.label }))}
              onChange={(v) => onChange({ pdfDpi: Number(v) })}
            />
          </div>
        </Row>
      </Group>

      <Group title="Word, PowerPoint and Excel to PDF">
        {OFFICE_ROWS.map((row) => (
          <Row
            key={row.key}
            label={row.label}
            description="Auto uses the best way available. If the app you pick isn't installed, Auto is used."
          >
            <div className="w-[264px]">
              <Segmented
                label={row.label}
                value={settings.office?.[row.key] ?? "auto"}
                choices={row.choices}
                onChange={(v) =>
                  onChange({ office: { ...settings.office, [row.key]: v === "auto" ? null : (v as Engine) } })
                }
              />
            </div>
          </Row>
        ))}
      </Group>
    </>
  );
}
