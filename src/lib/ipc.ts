// The ONLY file that calls invoke()/listen() or Tauri plugin APIs. Every Rust command gets a typed wrapper here.
import { invoke, type InvokeArgs } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open } from "@tauri-apps/plugin-dialog";
import type {
  AppError,
  EditorPreset,
  Estimate,
  FileSet,
  JobDone,
  JobProgress,
  JobRequest,
  PopupFilesPayload,
  PdfPageSize,
  PopupShowPayload,
  Preset,
  Preview,
  RotateCheck,
  PdfEstimate,
  PdfLevel,
  Turn,
  Settings,
  SettingsView,
} from "./types";

export function isAppError(value: unknown): value is AppError {
  return (
    typeof value === "object" &&
    value !== null &&
    typeof (value as AppError).code === "string" &&
    typeof (value as AppError).message === "string"
  );
}

/** Invoke a command and normalise any rejection into an `AppError`. */
async function call<T>(command: string, args?: InvokeArgs): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (err) {
    if (isAppError(err)) throw err;
    const fallback: AppError = { code: "internal", message: "Something went wrong. Please try again." };
    throw fallback;
  }
}

// ---- Popup ----

export const hidePopup = () => call<void>("hide_popup");

/** Tell Rust the popup painted its first frame (dev latency log). */
export const popupShown = () => call<void>("popup_shown");

/** Whether native blur is active; false → use the solid background. */
export const blurEnabled = () => call<boolean>("blur_enabled");

/** Fit the window to the content (logical px). Editors pass a wider width. */
export const resizePopup = (width: number, height: number) => call<void>("resize_popup", { width, height });

/** Start moving the popup with the mouse (called on mouse-down on its header). */
export const dragPopup = () => call<void>("drag_popup");

export const focusPopup = () => call<void>("focus_popup");

export const onPopupShow = (handler: (payload: PopupShowPayload) => void): Promise<UnlistenFn> =>
  listen<PopupShowPayload>("popup://show", (event) => handler(event.payload));

export const onPopupFiles = (handler: (payload: PopupFilesPayload) => void): Promise<UnlistenFn> =>
  listen<PopupFilesPayload>("popup://files", (event) => handler(event.payload));

// ---- Files ----

export const describeFiles = (paths: string[]) => call<FileSet>("describe_files", { paths });

/** Small PNG data URL for an image file. */
export const thumbnail = (path: string) => call<string>("thumbnail", { path });

/** Rotate view: thumbnail (longest side `size` px) and which way is up, if it can tell. */
export const rotateCheck = (path: string, size: number) => call<RotateCheck>("rotate_check", { path, size });

/** Page tray: a sharp thumbnail and the photo's upright proportions. */
export const trayThumbnail = (path: string) => call<Preview>("tray_thumbnail", { path });

/** Make PDF smaller: the result size for `level`, measured without saving. */
export const pdfCompressEstimate = (path: string, level: PdfLevel) =>
  call<PdfEstimate>("pdf_compress_estimate", { path, level });

/** Rotate pages: clockwise degrees that make the page straight (as it shows now), or null. */
export const pdfPageUpright = (path: string, page: number) => call<Turn | null>("pdf_page_upright", { path, page });

/** Larger preview for editors, plus the image's upright size in px. */
export const preview = (path: string) => call<Preview>("preview", { path });

/** Size of every page of a PDF, in points. Rejects with a friendly error for locked/damaged PDFs. */
export const pdfPageSizes = (path: string) => call<PdfPageSize[]>("pdf_page_sizes", { path });

/** JPEG data URLs for the given 1-based pages. */
export const pdfThumbnails = (path: string, pages: number[]) => call<string[]>("pdf_thumbnails", { path, pages });


const DIALOG_EXTENSIONS = ["jpg", "jpeg", "png", "webp", "bmp", "gif", "tif", "tiff", "heic", "heif", "pdf"];

/** Native "Choose files…" dialog. Resolves to [] when cancelled. */
export async function chooseFiles(): Promise<string[]> {
  const picked = await open({
    multiple: true,
    directory: false,
    filters: [{ name: "Images and PDFs", extensions: DIALOG_EXTENSIONS }],
  });
  if (picked === null) return [];
  return Array.isArray(picked) ? picked : [picked];
}

export type DropEvent = { type: "over" } | { type: "leave" } | { type: "drop"; paths: string[] };

/** Files dragged onto / dropped on the popup. */
export const onFileDrop = (handler: (event: DropEvent) => void): Promise<UnlistenFn> =>
  getCurrentWebview().onDragDropEvent(({ payload }) => {
    if (payload.type === "drop") handler({ type: "drop", paths: payload.paths });
    else if (payload.type === "leave") handler({ type: "leave" });
    else handler({ type: "over" });
  });

// ---- Jobs ----

/** Start a conversion; resolves to the job id. Progress and the result arrive as events. */
export const startJob = (request: JobRequest, paths: string[]) => call<string>("start_job", { request, paths });

export const cancelJob = (jobId: string) => call<void>("cancel_job", { jobId });

export const onJobProgress = (handler: (progress: JobProgress) => void): Promise<UnlistenFn> =>
  listen<JobProgress>("job://progress", (event) => handler(event.payload));

export const onJobDone = (handler: (done: JobDone) => void): Promise<UnlistenFn> =>
  listen<JobDone>("job://done", (event) => handler(event.payload));

/** Open a file written by a job in its default app. */
export const openOutput = (path: string) => call<void>("open_output", { path });

/** Shipped presets merged with the user's own. */
export const listPresets = () => call<Preset[]>("list_presets");

/** What Compress / Apply preset would produce for one image, without writing anything. */
export const estimateOutput = (path: string, request: JobRequest) => call<Estimate>("estimate_output", { path, request });

/** Show a file written by a job in Finder/Explorer. */
export const revealOutput = (path: string) => call<void>("reveal_output", { path });

// ---- Settings ----

export const getSettings = () => call<SettingsView>("get_settings");

/** Save and apply everything except the shortcut. */
export const saveSettings = (settings: Settings) => call<SettingsView>("save_settings", { settings });

/** Change the global shortcut (null = default). Rejects with a plain message if it can't be used. */
export const setShortcut = (accelerator: string | null) => call<SettingsView>("set_shortcut", { accelerator });

/** Pause the global shortcut while recording a new one. */
export const suspendShortcut = (paused: boolean) => call<void>("suspend_shortcut", { paused });

export const setLaunchAtLogin = (enabled: boolean) => call<SettingsView>("set_launch_at_login", { enabled });

export const openSettings = (tab?: "general" | "conversions" | "presets") => call<void>("open_settings", { tab });

export const onSettingsChanged = (handler: (view: SettingsView) => void): Promise<UnlistenFn> =>
  listen<SettingsView>("settings://changed", (event) => handler(event.payload));

export const presetsForEditor = () => call<EditorPreset[]>("presets_for_editor");

export const savePresets = (presets: Preset[]) => call<EditorPreset[]>("save_presets", { presets });

export const restorePresets = () => call<EditorPreset[]>("restore_presets");

/** Native folder picker. Resolves to null when cancelled. */
export async function chooseFolder(): Promise<string | null> {
  const picked = await open({ directory: true, multiple: false });
  return typeof picked === "string" ? picked : null;
}

/** Apply System / Light / Dark to this window's CSS (`data-theme` on <html>). */
export function applyAppearance(appearance: Settings["appearance"]) {
  if (appearance === "system") delete document.documentElement.dataset.theme;
  else document.documentElement.dataset.theme = appearance;
}
