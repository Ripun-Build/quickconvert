// The ONLY file that calls invoke()/listen() or Tauri plugin APIs. Every Rust command gets a typed wrapper here.
import { invoke, type InvokeArgs } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open } from "@tauri-apps/plugin-dialog";
import type { AppError, FileSet, PopupFilesPayload, PopupShowPayload } from "./types";

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

/** Fit the window to the content height (logical px). */
export const resizePopup = (height: number) => call<void>("resize_popup", { height });

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
