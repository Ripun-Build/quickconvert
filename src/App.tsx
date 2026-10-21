// Popup root: listens for show/files/job events, runs appear/hide motion, and switches between views.
import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { AnimatePresence, MotionConfig, motion, useReducedMotion, type Transition } from "framer-motion";
import { Settings } from "lucide-react";
import { FileChip } from "./components/FileChip";
import { ACTION_SPECS, type ActionDef, type ActionSpec, type InputKind, type OptionValues } from "./lib/actions";
import {
  blurEnabled,
  cancelJob,
  chooseFiles,
  describeFiles,
  focusPopup,
  hidePopup,
  isAppError,
  onFileDrop,
  onJobDone,
  onJobProgress,
  onPopupFiles,
  onPopupShow,
  popupShown,
  resizePopup,
  setPopupPinned,
  startJob,
} from "./lib/ipc";
import type { FileInfo, FileSet, JobDone, JobRequest, PopupOrigin } from "./lib/types";
import { ActionList } from "./views/ActionList";
import { DropZone, type DropZoneHint } from "./views/DropZone";
import { ErrorView } from "./views/ErrorView";
import { OptionsSheet } from "./views/OptionsSheet";
import { CompressView } from "./views/CompressView";
import { PdfPagesEditor } from "./views/PdfPagesEditor";
import { PresetView } from "./views/PresetView";
import { ResizeEditor } from "./views/ResizeEditor";
import { Result } from "./views/Result";
import { Running } from "./views/Running";

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
const PUSH: Transition = { type: "spring", stiffness: 420, damping: 36 };
const pushVariants = {
  enter: (dir: number) => ({ x: dir * 24, opacity: 0 }),
  center: { x: 0, opacity: 1 },
  exit: (dir: number) => ({ x: dir * -24, opacity: 0 }),
};

const POPUP_WIDTH = 360;
const EDITOR_WIDTH = 720;

type Phase = "hidden" | "open" | "closing";
type View =
  | { name: "loading" }
  | { name: "dropzone"; hint: DropZoneHint }
  | { name: "actions"; set: FileSet }
  | { name: "options"; set: FileSet; action: ActionDef; spec: ActionSpec; files: FileInfo[] }
  | { name: "resize"; set: FileSet; action: ActionDef; files: FileInfo[] }
  | { name: "pages"; set: FileSet; action: ActionDef; files: FileInfo[] }
  | { name: "compress"; set: FileSet; action: ActionDef; files: FileInfo[] }
  | { name: "preset"; set: FileSet; action: ActionDef; files: FileInfo[] }
  | {
      name: "running";
      set: FileSet;
      action: ActionDef;
      index: number;
      count: number;
      fileName: string;
      percent: number;
      cancelling: boolean;
    }
  | { name: "result"; set: FileSet; done: JobDone }
  | { name: "error"; set: FileSet; done: JobDone };

/** Depth in the push/pop stack; decides the slide direction between views. */
const DEPTH: Record<View["name"], number> = {
  loading: 0,
  dropzone: 0,
  actions: 0,
  options: 1,
  resize: 1,
  pages: 1,
  compress: 1,
  preset: 1,
  running: 2,
  result: 2,
  error: 2,
};

const FOOTER: Record<View["name"], string> = {
  loading: "esc close",
  dropzone: "esc close",
  actions: "↑↓ navigate · ↵ select · esc close",
  options: "↵ run · esc back",
  resize: "drag the handles or type a size · ↵ resize · esc back",
  pages: "click pages to pick · shift-click for a range · esc back",
  compress: "↵ compress · esc back",
  preset: "↵ apply · esc back",
  running: "esc cancel",
  result: "esc close",
  error: "esc back",
};

interface LastJob {
  action: ActionDef;
  request: JobRequest;
  files: FileInfo[];
  set: FileSet;
}

function App() {
  const reduceMotion = useReducedMotion();
  const [phase, setPhase] = useState<Phase>("hidden");
  const [showCount, setShowCount] = useState(0);
  const [origin, setOrigin] = useState<PopupOrigin>("top-left");
  const [hotkey, setHotkey] = useState("");
  const [view, setView] = useState<View>({ name: "dropzone", hint: undefined });
  const [direction, setDirection] = useState(0);
  const [dragging, setDragging] = useState(false);
  const [notice, setNotice] = useState<string>();
  const generation = useRef(0);
  const contentRef = useRef<HTMLDivElement>(null);
  const viewRef = useRef(view);
  viewRef.current = view;
  const jobId = useRef<string | null>(null);
  const earlyDone = useRef(new Map<string, JobDone>());
  const lastJob = useRef<LastJob | null>(null);

  /** Change view, sliding forward or back by stack depth. */
  const go = useCallback((next: View) => {
    const from = DEPTH[viewRef.current.name];
    const to = DEPTH[next.name];
    setDirection(to === from ? 0 : to > from ? 1 : -1);
    setView(next);
  }, []);

  /** Show the files, or the drop zone when none are supported. */
  const showFiles = useCallback(
    (set: FileSet) => go(set.files.length > 0 ? { name: "actions", set } : { name: "dropzone", hint: "unsupported" }),
    [go],
  );

  const openPaths = useCallback(
    async (paths: string[]) => {
      if (paths.length === 0 || viewRef.current.name === "running") return;
      try {
        showFiles(await describeFiles(paths));
        setNotice(undefined);
        await focusPopup();
      } catch {
        go({ name: "dropzone", hint: "unsupported" });
      }
    },
    [go, showFiles],
  );

  const finish = useCallback(
    (done: JobDone) => {
      jobId.current = null;
      const set = lastJob.current?.set ?? { files: [], unsupported: 0 };
      const ok = !done.cancelled && done.failures.length === 0 && done.outputs.length > 0;
      go(ok ? { name: "result", set, done } : { name: "error", set, done });
    },
    [go],
  );

  const launch = useCallback(
    async (job: LastJob) => {
      lastJob.current = job;
      go({
        name: "running",
        set: job.set,
        action: job.action,
        index: 0,
        count: job.files.length,
        fileName: job.files[0]?.name ?? "",
        percent: 0,
        cancelling: false,
      });
      try {
        const id = await startJob(
          job.request,
          job.files.map((f) => f.path),
        );
        jobId.current = id;
        const done = earlyDone.current.get(id);
        if (done) {
          earlyDone.current.delete(id);
          finish(done);
        }
      } catch (err) {
        const error = isAppError(err) ? err : { code: "internal" as const, message: "Something went wrong." };
        finish({
          jobId: "",
          outputs: [],
          failures: [{ name: job.files[0]?.name ?? "", error }],
          savedToFallback: false,
          cancelled: false,
        });
      }
    },
    [finish, go],
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
        setDirection(0);
        setView(payload.reading ? { name: "loading" } : { name: "dropzone", hint: undefined });
        setNotice(undefined);
        setPhase("open");
        setShowCount((n) => n + 1); // remount content so the appear animation replays
      }),
      onPopupFiles((payload) => {
        if (payload.generation !== generation.current) return;
        if (payload.state === "files") showFiles(payload.set);
        else go({ name: "dropzone", hint: payload.state === "permission" ? "permission" : undefined });
      }),
      onFileDrop((event) => {
        setDragging(event.type === "over");
        if (event.type === "drop") void openPaths(event.paths);
      }),
      onJobProgress((p) => {
        if (p.jobId !== jobId.current) return;
        setView((v) =>
          v.name === "running"
            ? { ...v, index: p.fileIndex, count: p.fileCount, percent: p.percent, fileName: p.fileName || v.fileName }
            : v,
        );
      }),
      onJobDone((done) => {
        // The result can beat startJob's reply for very quick jobs.
        if (done.jobId === jobId.current) finish(done);
        else earlyDone.current.set(done.jobId, done);
      }),
    ];
    return () => {
      unlisteners.forEach((p) => p.then((fn) => fn()));
    };
  }, [finish, go, openPaths, showFiles]);

  // Report first painted frame after each show (dev latency log).
  useEffect(() => {
    if (showCount === 0) return;
    requestAnimationFrame(() => requestAnimationFrame(() => void popupShown().catch(() => {})));
  }, [showCount]);

  // The drop zone stays open when focus moves to Finder (so files can be dragged in), and so does a running job.
  useEffect(() => {
    void setPopupPinned(phase === "open" && (view.name === "dropzone" || view.name === "running")).catch(() => {});
  }, [phase, view.name]);

  // Fit the window to the content: editors are wider (SPECS §5), height follows the content.
  const wide = view.name === "resize" || view.name === "pages";
  useLayoutEffect(() => {
    const el = contentRef.current;
    if (!el) return;
    let frame = 0;
    const fit = () => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(
        () => void resizePopup(wide ? EDITOR_WIDTH : POPUP_WIDTH, Math.ceil(el.offsetHeight)).catch(() => {}),
      );
    };
    fit();
    const observer = new ResizeObserver(fit);
    observer.observe(el);
    return () => {
      cancelAnimationFrame(frame);
      observer.disconnect();
    };
  }, [showCount, wide]);

  const close = useCallback(() => setPhase((p) => (p === "open" ? "closing" : p)), []);

  const cancel = useCallback(() => {
    const id = jobId.current;
    if (!id) return;
    setView((v) => (v.name === "running" ? { ...v, cancelling: true } : v));
    void cancelJob(id).catch(() => {});
  }, []);

  const back = useCallback(() => {
    const v = viewRef.current;
    if ("set" in v) go({ name: "actions", set: v.set });
  }, [go]);

  // Esc: back one view, cancel a running job, or close.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      const name = viewRef.current.name;
      if (["options", "resize", "pages", "compress", "preset", "error"].includes(name)) back();
      else if (name === "running") cancel();
      else close();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [back, cancel, close]);

  const choose = () => {
    chooseFiles()
      .then(openPaths)
      .catch(() => {});
  };

  const run = (set: FileSet) => (action: ActionDef, scope?: InputKind) => {
    const spec = ACTION_SPECS[action.id];
    if (!spec) {
      setNotice(`${action.label} isn't available yet.`);
      return;
    }
    setNotice(undefined);
    const files = scope ? set.files.filter((f) => f.kind === scope) : set.files;
    if (action.id === "image_resize") go({ name: "resize", set, action, files });
    else if (action.id === "pdf_to_images") go({ name: "pages", set, action, files });
    else if (action.id === "image_compress") go({ name: "compress", set, action, files });
    else if (action.id === "apply_preset") go({ name: "preset", set, action, files });
    else if (spec.fields.length === 0) void launch({ action, request: spec.toRequest(spec.defaults), files, set });
    else go({ name: "options", set, action, spec, files });
  };

  const retry = () => {
    const job = lastJob.current;
    const v = viewRef.current;
    if (!job || v.name !== "error") return;
    // Per-file actions retry only the files that failed; combined outputs retry everything.
    const failed = new Set(v.done.failures.map((f) => f.name));
    const perFile = ["image_convert", "image_resize", "image_compress", "apply_preset"].includes(job.request.action);
    const files = perFile ? job.files.filter((f) => failed.has(f.name)) : job.files;
    void launch({ ...job, files: files.length > 0 ? files : job.files });
  };

  const hiddenState = reduceMotion ? { opacity: 0 } : { opacity: 0, scale: 0.94, y: 4 };
  const closingState = reduceMotion ? { opacity: 0 } : { opacity: 0, scale: 0.97 };
  const chipSet = "set" in view ? view.set : undefined;

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
          className={`flex flex-col ${wide ? "max-h-[560px]" : "max-h-[480px]"}`}
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
            {chipSet ? <FileChip set={chipSet} /> : <h1 className="type-title text-primary">QuickConvert</h1>}
            <button
              type="button"
              disabled
              aria-label="Settings"
              className="flex size-7 shrink-0 items-center justify-center rounded-control text-secondary disabled:opacity-40"
            >
              <Settings size={18} strokeWidth={1.75} />
            </button>
          </header>

          <div className="relative flex min-h-0 flex-col">
            <AnimatePresence mode="popLayout" initial={false} custom={direction}>
              <motion.div
                key={`${view.name}-${showCount}`}
                custom={direction}
                variants={pushVariants}
                initial="enter"
                animate="center"
                exit="exit"
                transition={PUSH}
                className="flex min-h-0 flex-col"
              >
                <Body
                  view={view}
                  hotkey={hotkey}
                  dragging={dragging}
                  onChoose={choose}
                  onRun={run}
                  onBack={back}
                  onEditorRun={(v, request) => void launch({ action: v.action, request, files: v.files, set: v.set })}
                  onLaunch={(v, values) =>
                    void launch({ action: v.action, request: v.spec.toRequest(values), files: v.files, set: v.set })
                  }
                  onCancel={cancel}
                  onRetry={retry}
                  onAutoHide={close}
                />
              </motion.div>
            </AnimatePresence>
          </div>

          <footer className="type-caption flex h-7 shrink-0 items-center border-t border-separator px-4 text-tertiary">
            {notice ?? FOOTER[view.name]}
          </footer>
        </motion.div>
      </div>
    </MotionConfig>
  );
}

interface BodyProps {
  view: View;
  hotkey: string;
  dragging: boolean;
  onChoose: () => void;
  onRun: (set: FileSet) => (action: ActionDef, scope?: InputKind) => void;
  onBack: () => void;
  onLaunch: (view: Extract<View, { name: "options" }>, values: OptionValues) => void;
  /** Run a request built by an editor view (resize, PDF pages). */
  onEditorRun: (view: Extract<View, { name: "resize" | "pages" | "compress" | "preset" }>, request: JobRequest) => void;
  onCancel: () => void;
  onRetry: () => void;
  onAutoHide: () => void;
}

function Body(props: BodyProps) {
  const { view, hotkey, dragging, onChoose, onRun, onBack, onLaunch, onEditorRun, onCancel, onRetry, onAutoHide } = props;
  switch (view.name) {
    case "loading":
      return <LoadingRows />;
    case "dropzone":
      return <DropZone hint={view.hint} hotkey={hotkey} dragging={dragging} onChoose={onChoose} />;
    case "actions":
      return <ActionList set={view.set} onRun={onRun(view.set)} />;
    case "options":
      return (
        <OptionsSheet
          action={view.action}
          spec={view.spec}
          fileCount={view.files.length}
          onBack={onBack}
          onRun={(values) => onLaunch(view, values)}
        />
      );
    case "resize":
      return <ResizeEditor files={view.files} onBack={onBack} onRun={(request) => onEditorRun(view, request)} />;
    case "compress":
      return <CompressView files={view.files} onBack={onBack} onRun={(request) => onEditorRun(view, request)} />;
    case "preset":
      return <PresetView files={view.files} onBack={onBack} onRun={(request) => onEditorRun(view, request)} />;
    case "pages":
      return <PdfPagesEditor file={view.files[0]} onBack={onBack} onRun={(request) => onEditorRun(view, request)} />;
    case "running":
      return (
        <Running
          title={view.action.label}
          index={view.index}
          count={view.count}
          fileName={view.fileName}
          percent={view.percent}
          cancelling={view.cancelling}
          onCancel={onCancel}
        />
      );
    case "result":
      return <Result done={view.done} onAutoHide={onAutoHide} />;
    case "error":
      return <ErrorView done={view.done} onRetry={onRetry} onBack={onBack} />;
  }
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
