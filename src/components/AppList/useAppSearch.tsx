import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { AppEntry, AppSearchStatus } from "../../types";
import { useRequestGuard } from "../../hooks/useRequestGuard";

const DEBOUNCE_MS = 120;

function searchApps(query: string): Promise<AppEntry[]> {
  return invoke<AppEntry[]>("search_apps", { query });
}

export function useAppSearch(query: string) {
  const [apps, setApps] = useState<AppEntry[]>([]);
  const [status, setStatus] = useState<AppSearchStatus>("idle");
  const [resolvedQuery, setResolvedQuery] = useState("");
  const guard = useRequestGuard();

  useEffect(() => {
    const trimmed = query.trim();
    const id = guard.next();

    if (!trimmed) {
      setApps([]);
      setStatus("idle");
      setResolvedQuery("");
      return;
    }

    setStatus("loading");

    const timer = window.setTimeout(() => {
      searchApps(trimmed)
        .then((results) => {
          if (!guard.isCurrent(id)) return;
          setApps(results);
          setResolvedQuery(trimmed);
          setStatus("ready");
        })
        .catch(() => {
          if (!guard.isCurrent(id)) return;
          setApps([]);
          setResolvedQuery(trimmed);
          setStatus("ready");
        });
    }, DEBOUNCE_MS);

    return () => window.clearTimeout(timer);
  }, [query]);

  return { apps, status, resolvedQuery };
}
