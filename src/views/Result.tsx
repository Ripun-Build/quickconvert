// Success (SPECS §5): check draws in, "Saved", output name(s), Show in folder · Open · Copy path.
// Auto-hides after 2.5 s unless hovered or focused.
import { useEffect, useState } from "react";
import { motion } from "framer-motion";
import { Copy, FolderOpen } from "lucide-react";
import { Button } from "../components/Button";
import { describeFonts, describeMadeWith, describeOutput, describeRemoved, formatBytes, truncateMiddle } from "../lib/format";
import { openOutput, revealOutput } from "../lib/ipc";
import type { JobDone } from "../lib/types";

const AUTO_HIDE_MS = 2500;
const baseName = (path: string) => path.split(/[\\/]/).pop() ?? path;
const parentOf = (path: string) => path.slice(0, Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\")));

/** ` in “report pages”` when every output sits in the same folder. */
function sharedFolder(outputs: string[]): string {
  const dir = parentOf(outputs[0]);
  return dir && outputs.every((o) => parentOf(o) === dir) ? ` in “${truncateMiddle(baseName(dir), 28)}”` : "";
}

export function Result({ done, autoHide, onAutoHide }: { done: JobDone; autoHide: boolean; onAutoHide: () => void }) {
  const [engaged, setEngaged] = useState(false);
  const [copied, setCopied] = useState(false);
  const outputs = done.outputs.map((o) => o.path);
  const single = outputs.length === 1;
  const total = done.outputs.reduce((sum, o) => sum + o.bytes, 0);
  const removed = describeRemoved(done.outputs);
  const madeWith = describeMadeWith(done.outputs);
  const fonts = describeFonts(done.outputs);
  // Make PDF smaller: files that were already about as small as they get (nothing saved for them).
  const unchanged = done.unchanged ?? [];
  const unchangedNote =
    unchanged.length === 0
      ? undefined
      : unchanged.length === 1
        ? `“${truncateMiddle(unchanged[0], 32)}” was already about as small as it gets, so no copy was saved.`
        : `${unchanged.length} PDFs were already about as small as they get, so no copies were saved.`;

  useEffect(() => {
    // Stay open while the user is pointing at it, or when there's a note to read.
    if (!autoHide || engaged || done.savedToFallback) return;
    const timer = setTimeout(onAutoHide, AUTO_HIDE_MS);
    return () => clearTimeout(timer);
  }, [autoHide, engaged, done.savedToFallback, onAutoHide]);

  const copy = () => {
    navigator.clipboard
      .writeText(outputs.join("\n"))
      .then(() => {
        setCopied(true);
        setTimeout(() => setCopied(false), 1200);
      })
      .catch(() => {});
  };

  if (outputs.length === 0) {
    return (
      <div
        className="flex flex-col gap-3 px-4 pb-3"
        onMouseEnter={() => setEngaged(true)}
        onMouseLeave={() => setEngaged(false)}
        onFocus={() => setEngaged(true)}
        aria-live="polite"
      >
        <div className="flex items-center gap-3">
          <CheckMark />
          <div className="min-w-0">
            <h2 className="type-title text-primary">Already small</h2>
            <p className="type-caption text-secondary">{unchangedNote}</p>
          </div>
        </div>
        <Button onClick={onAutoHide}>Done</Button>
      </div>
    );
  }

  return (
    <div
      className="flex flex-col gap-3 px-4 pb-3"
      onMouseEnter={() => setEngaged(true)}
      onMouseLeave={() => setEngaged(false)}
      onFocus={() => setEngaged(true)}
      aria-live="polite"
    >
      <div className="flex items-center gap-3">
        <CheckMark />
        <div className="min-w-0">
          <h2 className="type-title text-primary">Saved</h2>
          <p className="type-caption truncate text-secondary" title={outputs.join("\n")}>
            {single ? truncateMiddle(baseName(outputs[0]), 40) : `${outputs.length} files${sharedFolder(outputs)}`}
          </p>
          <p className="type-caption truncate text-tertiary tabular-nums">
            {single ? describeOutput(done.outputs[0]) : `${formatBytes(total)} in total`}
          </p>
          {removed && <p className="type-caption text-secondary">{removed}</p>}
          {madeWith && <p className="type-caption text-secondary">{madeWith}</p>}
          {fonts && <p className="type-caption text-secondary">{fonts}</p>}
          {unchangedNote && <p className="type-caption text-secondary">{unchangedNote}</p>}
        </div>
      </div>
      {done.savedToFallback && (
        <p className="type-caption text-caution">Couldn't save in this folder, so the file was saved to Downloads.</p>
      )}
      <div className="flex gap-2">
        <Button className="flex-auto whitespace-nowrap" onClick={() => void revealOutput(outputs[0]).catch(() => {})}>
          <FolderOpen size={14} strokeWidth={1.75} />
          Show in folder
        </Button>
        {single && (
          <Button className="flex-auto whitespace-nowrap" onClick={() => void openOutput(outputs[0]).catch(() => {})}>
            Open
          </Button>
        )}
        <Button className="flex-auto whitespace-nowrap" onClick={copy}>
          <Copy size={14} strokeWidth={1.75} />
          {copied ? "Copied" : single ? "Copy path" : "Copy paths"}
        </Button>
      </div>
    </div>
  );
}

/** Green check whose stroke draws in (280 ms), then pops (DESIGN §5). */
function CheckMark() {
  return (
    <motion.svg
      width="32"
      height="32"
      viewBox="0 0 32 32"
      className="shrink-0 text-success"
      initial={{ scale: 1 }}
      animate={{ scale: [1, 1.06, 1] }}
      transition={{ delay: 0.28, duration: 0.24 }}
      aria-hidden
    >
      <circle cx="16" cy="16" r="15" fill="currentColor" opacity="0.15" />
      <motion.path
        d="M10 16.5l4 4 8-9"
        fill="none"
        stroke="currentColor"
        strokeWidth="2.5"
        strokeLinecap="round"
        strokeLinejoin="round"
        initial={{ pathLength: 0 }}
        animate={{ pathLength: 1 }}
        transition={{ duration: 0.28, ease: "easeOut" }}
      />
    </motion.svg>
  );
}
