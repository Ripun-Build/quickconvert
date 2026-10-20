// Popup root: listens for show events, runs appear/hide motion, and hosts the current view.
import { useCallback, useEffect, useState } from "react";
import { motion, useReducedMotion, type Transition } from "framer-motion";
import { Images, Settings } from "lucide-react";
import { blurEnabled, hidePopup, onPopupShow, popupShown } from "./lib/ipc";
import type { PopupOrigin } from "./lib/types";

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

function App() {
  const reduceMotion = useReducedMotion();
  const [phase, setPhase] = useState<Phase>("hidden");
  const [showCount, setShowCount] = useState(0);
  const [origin, setOrigin] = useState<PopupOrigin>("top-left");
  const [hotkey, setHotkey] = useState("");

  useEffect(() => {
    blurEnabled()
      .then((on) => {
        if (!on) document.documentElement.dataset.blur = "off";
      })
      .catch(() => {
        document.documentElement.dataset.blur = "off";
      });

    const unlisten = onPopupShow(({ origin, hotkey }) => {
      setOrigin(origin);
      setHotkey(hotkey);
      setPhase("open");
      setShowCount((n) => n + 1); // remount content so the appear animation replays
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  // Report first painted frame after each show (dev latency log).
  useEffect(() => {
    if (showCount === 0) return;
    requestAnimationFrame(() => requestAnimationFrame(() => void popupShown().catch(() => {})));
  }, [showCount]);

  const close = useCallback(() => setPhase((p) => (p === "open" ? "closing" : p)), []);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") close();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [close]);

  const hiddenState = reduceMotion ? { opacity: 0 } : { opacity: 0, scale: 0.94, y: 4 };
  const closingState = reduceMotion ? { opacity: 0 } : { opacity: 0, scale: 0.97 };

  return (
    <div
      role="dialog"
      aria-label="QuickConvert"
      className="h-screen w-screen overflow-hidden rounded-popup bg-glass inset-ring inset-ring-border-glass in-data-[blur=off]:bg-solid"
    >
      {/* Always mounted (invisible while hidden) so the first show doesn't pay for the first render. */}
      <motion.div
        key={showCount}
        aria-hidden={phase === "hidden"}
        className="flex h-full flex-col"
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
        <ShellPlaceholder hotkey={hotkey} />
      </motion.div>
    </div>
  );
}

/** M1 placeholder content. Replaced by DropZone / ActionList in M2. */
function ShellPlaceholder({ hotkey }: { hotkey: string }) {
  return (
    <>
      <header className="flex h-13 shrink-0 items-center justify-between px-4">
        <h1 className="type-title text-primary">QuickConvert</h1>
        <button
          type="button"
          disabled
          aria-label="Settings"
          className="flex size-7 items-center justify-center rounded-control text-secondary disabled:opacity-40"
        >
          <Settings size={18} strokeWidth={1.75} />
        </button>
      </header>
      <div className="flex flex-1 items-center gap-3 px-4">
        <Images size={18} strokeWidth={1.75} className="shrink-0 text-tertiary" />
        <p className="type-body text-secondary">
          Select files in Finder, then press {hotkey || "the shortcut"}.
        </p>
      </div>
      <footer className="type-caption flex h-7 shrink-0 items-center border-t border-separator px-4 text-tertiary">
        esc close
      </footer>
    </>
  );
}

export default App;
