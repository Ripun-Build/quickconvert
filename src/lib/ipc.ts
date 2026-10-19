// The ONLY file that calls invoke()/listen(). Every Rust command gets a typed wrapper here.
import { invoke, type InvokeArgs } from "@tauri-apps/api/core";
import type { AppError } from "./types";

export function isAppError(value: unknown): value is AppError {
  return (
    typeof value === "object" &&
    value !== null &&
    typeof (value as AppError).code === "string" &&
    typeof (value as AppError).message === "string"
  );
}

/** Invoke a command and normalise any rejection into an `AppError`. */
export async function call<T>(command: string, args?: InvokeArgs): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (err) {
    if (isAppError(err)) throw err;
    const fallback: AppError = { code: "internal", message: "Something went wrong. Please try again." };
    throw fallback;
  }
}
