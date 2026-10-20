// The ONLY file that calls invoke()/listen(). Every Rust command gets a typed wrapper here.
import { invoke, type InvokeArgs } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { AppError, PopupShowPayload } from "./types";

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

export const onPopupShow = (handler: (payload: PopupShowPayload) => void): Promise<UnlistenFn> =>
  listen<PopupShowPayload>("popup://show", (event) => handler(event.payload));
