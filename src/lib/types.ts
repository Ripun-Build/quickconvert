// TS mirrors of Rust structs. Keep in sync with src-tauri/src.

/** Mirrors `AppError` in src-tauri/src/errors.rs (serialised as `{ code, message }`). */
export type ErrorCode =
  | "io"
  | "unsupported"
  | "corrupt"
  | "password_protected"
  | "cancelled"
  | "invalid_input"
  | "missing_component"
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
  generation: number;
  origin: PopupOrigin;
  /** Display label of the registered hotkey, e.g. "⌃⇧Space". */
  hotkey: string;
  /** True while the Finder/Explorer selection is being read; `popup://files` follows. */
  reading: boolean;
}

/** `files::FileKind` */
export type FileKind = "image" | "pdf";

/** `files::FileInfo` */
export interface FileInfo {
  path: string;
  name: string;
  kind: FileKind;
  /** Bytes. */
  size: number;
}

/** `files::Preview`: editor preview data URL and the image's upright size in px. */
export interface Preview {
  url: string;
  width: number;
  height: number;
}

/** `pdf_render::PageSize`: a PDF page in points (1/72 inch). */
export interface PdfPageSize {
  width: number;
  height: number;
}

/** `files::FileSet`: supported files in selection order, plus how many inputs were skipped. */
export interface FileSet {
  files: FileInfo[];
  unsupported: number;
}

/** Payload of the `popup://files` event (`popup::FilesPayload`). */
export interface PopupFilesPayload {
  generation: number;
  state: "files" | "empty" | "permission";
  set: FileSet;
}

/** `convert::image::ResizeMode` */
export type ResizeMode =
  | { kind: "long_edge"; px: number }
  | { kind: "exact"; width: number; height: number; keepAspect: boolean }
  | { kind: "percent"; percent: number };

/** `crop::Turn`: clockwise degrees. */
export type Turn = 0 | 90 | 180 | 270;

/** `crop::CropRect`: pixels of the upright image after the turn. */
export interface CropRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** `upright::RotateCheck`: a thumbnail and the clockwise degrees that make the image straight. */
export interface RotateCheck {
  url: string;
  upright: Turn | null;
}

/** `compress::CompressTarget` */
export type CompressTarget = { kind: "size"; maxKb: number } | { kind: "quality"; quality: number };

/** `compress::Preset` (SPECS §8). Shipped presets are examples, never official specs. */
export interface Preset {
  id: string;
  label: string;
  format: "jpg" | "png" | "webp";
  width?: number;
  height?: number;
  aspect?: string;
  longEdge?: number;
  minKB?: number;
  maxKB?: number;
  dpi?: number;
  note?: string;
}

/** `commands::Estimate`: what Compress / a preset would produce for the first image. */
export interface Estimate {
  bytes: number;
  originalBytes: number;
  width: number;
  height: number;
  quality?: number | null;
}

/** `jobs::JobRequest`: what to run, with its options. */
export type JobRequest =
  | { action: "images_to_pdf"; pageSize: "fit" | "a4" | "letter"; margin: "none" | "small" }
  | { action: "pdf_merge" }
  | { action: "image_convert"; format: "jpg" | "png" | "webp"; quality: number }
  | { action: "image_resize"; mode: ResizeMode }
  | {
      action: "pdf_to_images";
      format: "jpg" | "png";
      dpi: number;
      pages: { kind: "all" } | { kind: "range"; spec: string };
    }
  | { action: "image_compress"; target: CompressTarget }
  | { action: "apply_preset"; preset: Preset }
  | { action: "image_crop"; rect: CropRect; turn: Turn }
  /** Each image's own turn, keyed by path. */
  | { action: "image_rotate"; turns: Record<string, Turn> };

/** Payload of the `job://progress` event. */
export interface JobProgress {
  jobId: string;
  /** Index of the file being processed; equals `fileCount` when finished. */
  fileIndex: number;
  fileCount: number;
  percent: number;
  fileName: string;
}

/** `jobs::Failure` */
export interface JobFailure {
  name: string;
  error: AppError;
}

/** `jobs::OutputFile`: a written file and, for images, what it ended up as. */
export interface OutputFile {
  path: string;
  bytes: number;
  width?: number;
  height?: number;
  quality?: number;
}

/** Payload of the `job://done` event (`jobs::JobDone`). */
export interface JobDone {
  jobId: string;
  outputs: OutputFile[];
  failures: JobFailure[];
  /** Some output went to Downloads because the folder wasn't writable. */
  savedToFallback: boolean;
  cancelled: boolean;
}

/** `settings::Settings` (SPECS §10). */
export interface Settings {
  /** e.g. "Control+Shift+Space"; null = the default. */
  shortcut: string | null;
  output: { kind: "next_to_original" } | { kind: "folder"; path: string };
  jpegQuality: number;
  resizeLongEdge: number;
  pdfDpi: number;
  appearance: "system" | "light" | "dark";
  autoHide: boolean;
}

/** `commands::SettingsView`: settings plus what the Settings window shows alongside them. */
export interface SettingsView {
  settings: Settings;
  shortcutLabel: string;
  defaultShortcutLabel: string;
  shortcutConflict: string | null;
  launchAtLogin: boolean;
}

/** `presets::EditorPreset` */
export interface EditorPreset {
  preset: Preset;
  /** Shipped with the app (an example), possibly edited. */
  builtin: boolean;
}
