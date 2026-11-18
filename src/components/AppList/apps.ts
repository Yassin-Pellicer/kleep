import { invoke } from "@tauri-apps/api/core";
import type { AppEntry } from "../../types";

export function searchApps(query: string): Promise<AppEntry[]> {
  return invoke<AppEntry[]>("search_apps", { query });
}

export function launchApp(id: string): Promise<void> {
  return invoke<void>("launch_app", { id });
}
