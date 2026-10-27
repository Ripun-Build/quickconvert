// File thumbnail: image preview from Rust, or a type icon (PDFs, loading, unreadable files).
import { useEffect, useState } from "react";
import { FileSpreadsheet, FileText, Image as ImageIcon, Presentation } from "lucide-react";
import { docType } from "../lib/office";
import { thumbnail } from "../lib/ipc";
import type { FileInfo } from "../lib/types";

const cache = new Map<string, string>();

export function Thumbnail({ file, className = "" }: { file: FileInfo; className?: string }) {
  const [src, setSrc] = useState<string | undefined>(() => cache.get(file.path));

  useEffect(() => {
    // Images and Office documents have previews; PDFs show an icon.
    if (file.kind === "pdf" || cache.has(file.path)) {
      setSrc(cache.get(file.path));
      return;
    }
    let live = true;
    thumbnail(file.path)
      .then((url) => {
        cache.set(file.path, url);
        if (live) setSrc(url);
      })
      .catch(() => {}); // keep the icon
    return () => {
      live = false;
    };
  }, [file.path, file.kind]);

  const type = file.kind === "document" ? docType(file.name) : null;
  const Icon = type === "slides" ? Presentation : type === "sheet" ? FileSpreadsheet : file.kind === "image" ? ImageIcon : FileText;
  return (
    <div
      className={`flex size-8 shrink-0 items-center justify-center overflow-hidden rounded-thumb bg-surface ring-[0.5px] ring-separator ${className}`}
    >
      {src ? (
        <img src={src} alt="" draggable={false} className="size-full object-cover" />
      ) : (
        <Icon size={16} strokeWidth={1.75} className="text-tertiary" />
      )}
    </div>
  );
}
