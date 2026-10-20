// TS mirrors of Rust structs. Keep in sync with src-tauri/src.

/** Mirrors `AppError` in src-tauri/src/errors.rs (serialised as `{ code, message }`). */
export type ErrorCode =
  | "io"
  | "unsupported"
  | "corrupt"
  | "password_protected"
  | "cancelled"
  | "internal";

export interface AppError {
  code: ErrorCode;
  /** Plain-language message, safe to show in the UI as-is. */
  message: string;
}

/** Popup corner nearest the cursor (`popup::position::Origin`); used as transform-origin. */
export type PopupOrigin = "top-left" | "top-right" | "bottom-left" | "bottom-right" | "center";

/** Payload of the `popup://show` event. */
export interface PopupShowPayload {
  origin: PopupOrigin;
  /** Display label of the registered hotkey, e.g. "⌃⇧Space". */
  hotkey: string;
}

/** Payload of the `job://progress` event. */
export interface JobProgress {
  jobId: string;
  fileIndex: number;
  fileCount: number;
  percent: number;
}
