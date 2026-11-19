import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type {
  DictionaryEntry,
  DictionaryStatus,
  LookupError,
} from "../../types";
import { useRequestGuard } from "../../hooks/useRequestGuard";
import { normalizeError } from "../../lib/normalizeError";

const DEBOUNCE_MS = 450;

function lookupWord(word: string): Promise<DictionaryEntry> {
  return invoke<DictionaryEntry>("lookup_word", { word });
}

// Entries never change for a given word, so keep them for the whole session.
const CACHE = new Map<string, DictionaryEntry>();

export function useDictionary(
  query: string,
  enabled: boolean,
  resetKey: number,
) {
  const [status, setStatus] = useState<DictionaryStatus>("idle");
  const [entry, setEntry] = useState<DictionaryEntry | null>(null);
  const [error, setError] = useState<LookupError | null>(null);
  const guard = useRequestGuard();

  // A fresh palette session always starts blank.
  useEffect(() => {
    guard.invalidate();
    setEntry(null);
    setError(null);
    setStatus("idle");
  }, [resetKey]);

  useEffect(() => {
    const id = guard.next();

    if (!enabled || !query) {
      // Keep the previous entry mounted so the collapse animation has content
      // to clip while the palette closes.
      setError(null);
      setStatus("idle");
      return;
    }

    setError(null);
    setStatus("loading");

    const timer = window.setTimeout(() => {
      const key = query.toLowerCase();
      const cached = CACHE.get(key);

      if (cached) {
        if (guard.isCurrent(id)) {
          setEntry(cached);
          setStatus("found");
        }
        return;
      }

      lookupWord(query)
        .then((result) => {
          if (!guard.isCurrent(id)) return;
          CACHE.set(key, result);
          setEntry(result);
          setStatus("found");
        })
        .catch((lookupError: unknown) => {
          if (!guard.isCurrent(id)) return;
          setError(normalizeError(lookupError));
          setStatus("error");
        });
    }, DEBOUNCE_MS);

    return () => window.clearTimeout(timer);
  }, [query, enabled, resetKey]);

  const isLoading =
    status === "loading" ||
    (status === "idle" && enabled && query.length > 0);

  return { status, entry, error, isLoading };
}
