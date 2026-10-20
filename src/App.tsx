// Popup root: listens for show/files events, runs appear/hide motion, and switches between views.
import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { MotionConfig, motion, useReducedMotion, type Transition } from "framer-motion";
import { Settings } from "lucide-react";
import { FileChip } from "./components/FileChip";
import type { ActionDef } from "./lib/actions";
import {
  blurEnabled,
  chooseFiles,
  describeFiles,
  focusPopup,
  hidePopup,
  onFileDrop,
  onPopupFiles,
  onPopupShow,
  popupShown,
  resizePopup,
  setPopupPinned,
} from "./lib/ipc";
import type { FileSet, PopupOrigin } from "./lib/types";
import { ActionList } from "./views/ActionList";
import { DropZone, type DropZoneHint } from "./views/DropZone";

const TRANSFORM_ORIGIN: Record<PopupOrigin, string> = {
  "top-left": "0% 0%",
  "top-right": "100% 0%",
  "bottom-left": "0% 100%",
  "bottom-right": "100% 100%",
  center: "50% 50%",
};

// DESIGN.md §5
const APPEAR: Transition = { type: "spring", stiffness: 520, damping: 34, mass: 0.8 };
const HIDE: Transition = { duration: 0.12, ease: "easeOut" };
const FADE: Transition = { duration: 0.12 };

type Phase = "hidden" | "open" | "closing";
type View =
  | { name: "loading" }
  | { name: "dropzone"; hint: DropZoneHint }
  | { name: "actions"; set: FileSet };

function App() {
  const reduceMotion = useReducedMotion();
  const [phase, setPhase] = useState<Phase>("hidden");
  const [showCount, setShowCount] = useState(0);
  const [origin, setOrigin] = useState<PopupOrigin>("top-left");
  const [hotkey, setHotkey] = useState("");
  const [view, setView] = useState<View>({ name: "dropzone", hint: undefined });
  const [dragging, setDragging] = useState(false);
  const [notice, setNotice] = useState<string>();
  const generation = useRef(0);
  const contentRef = useRef<HTMLDivElement>(null);

  /** Show the files, or the drop zone when none are supported. */
  const showFiles = useCallback((set: FileSet) => {
    setView(set.files.length > 0 ? { name: "actions", set } : { name: "dropzone", hint: "unsupported" });
  }, []);

  const openPaths = useCallback(
    async (paths: string[]) => {
      if (paths.length === 0) return;
      try {
        showFiles(await describeFiles(paths));
        setNotice(undefined);
        await focusPopup();
      } catch {
        setView({ name: "dropzone", hint: "unsupported" });
      }
    },
    [showFiles],
  );

  useEffect(() => {
    blurEnabled()
      .then((on) => {
        if (!on) document.documentElement.dataset.blur = "off";
      })
      .catch(() => {
        document.documentElement.dataset.blur = "off";
      });

    const unlisteners = [
      onPopupShow((payload) => {
        generation.current = payload.generation;
        setOrigin(payload.origin);
        setHotkey(payload.hotkey);
        setView(payload.reading ? { name: "loading" } : { name: "dropzone", hint: undefined });
        setNotice(undefined);
        setPhase("open");
        setShowCount((n) => n + 1); // remount content so the appear animation replays
      }),
      onPopupFiles((payload) => {
        if (payload.generation !== generation.current) return;
        if (payload.state === "files") showFiles(payload.set);
        else setView({ name: "dropzone", hint: payload.state === "permission" ? "permission" : undefined });
      }),
      onFileDrop((event) => {
        setDragging(event.type === "over");
        if (event.type === "drop") void openPaths(event.paths);
      }),
    ];
    return () => {
      unlisteners.forEach((p) => p.then((fn) => fn()));
    };
  }, [openPaths, showFiles]);

  // Report first painted frame after each show (dev latency log).
  useEffect(() => {
    if (showCount === 0) return;
    requestAnimationFrame(() => requestAnimationFrame(() => void popupShown().catch(() => {})));
  }, [showCount]);

  // The drop zone stays open when focus moves to Finder, so files can be dragged in.
  useEffect(() => {
    void setPopupPinned(phase === "open" && view.name === "dropzone").catch(() => {});
  }, [phase, view.name]);

  // Fit the window to the content height.
  useLayoutEffect(() => {
    const el = contentRef.current;
    if (!el) return;
    let frame = 0;
    const observer = new ResizeObserver(() => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => void resizePopup(Math.ceil(el.offsetHeight)).catch(() => {}));
    });
    observer.observe(el);
    return () => {
      cancelAnimationFrame(frame);
      observer.disconnect();
    };
  }, [showCount]);

  const close = useCallback(() => setPhase((p) => (p === "open" ? "closing" : p)), []);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") close();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [close]);

  const choose = () => {
    chooseFiles()
      .then(openPaths)
      .catch(() => {});
  };

  const run = (action: ActionDef) => {
    // Conversions arrive in M3.
    setNotice(`${action.label} isn't available yet.`);
  };

  const hiddenState = reduceMotion ? { opacity: 0 } : { opacity: 0, scale: 0.94, y: 4 };
  const closingState = reduceMotion ? { opacity: 0 } : { opacity: 0, scale: 0.97 };

  return (
    <MotionConfig reducedMotion="user">
      <div
        role="dialog"
        aria-label="QuickConvert"
        className="h-screen w-screen overflow-hidden rounded-popup bg-glass inset-ring inset-ring-border-glass in-data-[blur=off]:bg-solid"
      >
        {/* Always mounted (invisible while hidden) so the first show doesn't pay for the first render. */}
        <motion.div
          key={showCount}
          ref={contentRef}
          aria-hidden={phase === "hidden"}
          className="flex max-h-[480px] flex-col"
          style={{ transformOrigin: TRANSFORM_ORIGIN[origin] }}
          initial={hiddenState}
          animate={
            phase === "open" ? { opacity: 1, scale: 1, y: 0 } : phase === "closing" ? closingState : hiddenState
          }
          transition={reduceMotion ? FADE : phase === "closing" ? HIDE : APPEAR}
          onAnimationComplete={() => {
            if (phase !== "closing") return;
            setPhase("hidden");
            void hidePopup().catch(() => {});
          }}
        >
          <header className="flex h-13 shrink-0 items-center justify-between gap-3 px-4">
            {view.name === "actions" ? (
              <FileChip set={view.set} />
            ) : (
              <h1 className="type-title text-primary">QuickConvert</h1>
            )}
            <button
              type="button"
              disabled
              aria-label="Settings"
              className="flex size-7 shrink-0 items-center justify-center rounded-control text-secondary disabled:opacity-40"
            >
              <Settings size={18} strokeWidth={1.75} />
            </button>
          </header>

          {view.name === "loading" && <LoadingRows />}
          {view.name === "dropzone" && (
            <DropZone hint={view.hint} hotkey={hotkey} dragging={dragging} onChoose={choose} />
          )}
          {view.name === "actions" && <ActionList key={showCount} set={view.set} onRun={run} />}

          <footer className="type-caption flex h-7 shrink-0 items-center border-t border-separator px-4 text-tertiary">
            {notice ?? (view.name === "actions" ? "↑↓ navigate · ↵ select · esc close" : "esc close")}
          </footer>
        </motion.div>
      </div>
    </MotionConfig>
  );
}

/** Placeholder rows while the Finder selection is read (usually ~100 ms). */
function LoadingRows() {
  return (
    <div className="flex flex-col gap-1 px-3 pb-2" aria-label="Reading selection">
      <div className="skeleton h-8 rounded-control" />
      {[0, 1, 2, 3].map((i) => (
        <div key={i} className="skeleton h-8 rounded-row" />
      ))}
    </div>
  );
}

export default App;
