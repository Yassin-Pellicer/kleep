import { useEffect, useRef, useState } from "react";
import type { AppEntry, AppSearchStatus } from "../../types";
import { searchApps } from "./apps";

const DEBOUNCE_MS = 120;

export function useAppSearch(query: string) {
  const [apps, setApps] = useState<AppEntry[]>([]);
  const [status, setStatus] = useState<AppSearchStatus>("idle");
  const [resolvedQuery, setResolvedQuery] = useState("");
  const requestId = useRef(0);

  useEffect(() => {
    const trimmed = query.trim();
    const id = requestId.current + 1;
    requestId.current = id;

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
          if (requestId.current !== id) return;
          setApps(results);
          setResolvedQuery(trimmed);
          setStatus("ready");
        })
        .catch(() => {
          if (requestId.current !== id) return;
          setApps([]);
          setResolvedQuery(trimmed);
          setStatus("ready");
        });
    }, DEBOUNCE_MS);

    return () => window.clearTimeout(timer);
  }, [query]);

  return { apps, status, resolvedQuery };
}
