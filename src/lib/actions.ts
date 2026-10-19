// Action registry (SPECS §6). Entries and suggestion logic arrive in M2.

export type InputKind = "image" | "pdf";

export type ActionId =
  | "images_to_pdf"
  | "pdf_to_images"
  | "image_convert"
  | "image_resize"
  | "image_compress"
  | "pdf_merge"
  | "apply_preset";

export type ActionGroup = "convert" | "edit" | "compress";

export interface ActionDef {
  id: ActionId;
  label: string;
  group: ActionGroup;
  /** Input kind every selected file must have. */
  input: InputKind;
  /** Minimum number of files the action needs. */
  minFiles: number;
  keywords: string[];
}

export const ACTIONS: readonly ActionDef[] = [];
