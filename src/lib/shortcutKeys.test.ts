import { test } from "node:test";
import assert from "node:assert/strict";
import { liveLabel, toAccelerator } from "./shortcutKeys.ts";

const press = (code: string, mods: Partial<{ meta: boolean; ctrl: boolean; alt: boolean; shift: boolean }> = {}) => ({
  code,
  metaKey: !!mods.meta,
  ctrlKey: !!mods.ctrl,
  altKey: !!mods.alt,
  shiftKey: !!mods.shift,
});

test("key presses become accelerators Rust understands", () => {
  assert.equal(toAccelerator(press("Space", { ctrl: true, shift: true })), "Control+Shift+Space");
  assert.equal(toAccelerator(press("KeyK", { meta: true, alt: true })), "Alt+Super+KeyK");
  assert.equal(toAccelerator(press("Digit1", { ctrl: true })), "Control+Digit1");
});

test("holding only modifiers is not a shortcut yet", () => {
  assert.equal(toAccelerator(press("ShiftLeft", { shift: true })), null);
  assert.equal(liveLabel(press("ControlLeft", { ctrl: true, shift: true })), "⌃⇧…");
});

test("labels use Mac symbols in Apple's order", () => {
  assert.equal(liveLabel(press("KeyK", { meta: true, alt: true, ctrl: true, shift: true })), "⌃⌥⇧⌘K");
  assert.equal(liveLabel(press("Comma", { ctrl: true, alt: true })), "⌃⌥,");
  assert.equal(liveLabel(press("Space", { ctrl: true, shift: true })), "⌃⇧Space");
});
