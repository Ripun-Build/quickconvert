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

/** Payload of the `job://progress` event. */
export interface JobProgress {
  jobId: string;
  fileIndex: number;
  fileCount: number;
  percent: number;
}
