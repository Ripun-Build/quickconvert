// The ONLY file that calls invoke()/listen() or Tauri plugin APIs. Every Rust command gets a typed wrapper here.
import { invoke, type InvokeArgs } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open } from "@tauri-apps/plugin-dialog";
import type {
  AppError,
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

/** While pinned (drop zone), clicking elsewhere doesn't hide the popup. */
export const setPopupPinned = (pinned: boolean) => call<void>("set_popup_pinned", { pinned });

export const focusPopup = () => call<void>("focus_popup");

export const onPopupShow = (handler: (payload: PopupShowPayload) => void): Promise<UnlistenFn> =>
  listen<PopupShowPayload>("popup://show", (event) => handler(event.payload));

export const onPopupFiles = (handler: (payload: PopupFilesPayload) => void): Promise<UnlistenFn> =>
  listen<PopupFilesPayload>("popup://files", (event) => handler(event.payload));

// ---- Files ----

export const describeFiles = (paths: string[]) => call<FileSet>("describe_files", { paths });

/** Small PNG data URL for an image file. */
export const thumbnail = (path: string) => call<string>("thumbnail", { path });

/** Larger preview for editors, plus the image's upright size in px. */
export const preview = (path: string) => call<Preview>("preview", { path });

/** Size of every page of a PDF, in points. Rejects with a friendly error for locked/damaged PDFs. */
export const pdfPageSizes = (path: string) => call<PdfPageSize[]>("pdf_page_sizes", { path });

/** JPEG data URLs for the given 1-based pages. */
export const pdfThumbnails = (path: string, pages: number[]) => call<string[]>("pdf_thumbnails", { path, pages });


const DIALOG_EXTENSIONS = ["jpg", "jpeg", "png", "webp", "bmp", "gif", "tif", "tiff", "pdf"];

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
