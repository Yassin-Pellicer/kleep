import { invoke } from "@tauri-apps/api/core";
import type { ClipboardEntry } from "../../types";

export function clipboardList(): Promise<ClipboardEntry[]> {
  return invoke<ClipboardEntry[]>("clipboard_list");
}

export function clipboardCopy(id: string): Promise<void> {
  return invoke<void>("clipboard_copy", { id });
}

export function clipboardDelete(id: string): Promise<void> {
  return invoke<void>("clipboard_delete", { id });
}

export function clipboardPaste(id: string): Promise<void> {
  return invoke<void>("clipboard_paste", { id });
}

export function clipboardClear(): Promise<void> {
  return invoke<void>("clipboard_clear");
}
