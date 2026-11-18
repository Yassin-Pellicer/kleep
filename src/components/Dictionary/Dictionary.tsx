import { useDictionary } from "./useDictionary";
import type { DictionaryEntry, LookupError } from "../../types";
import "./Dictionary.css";

const ERROR_MESSAGES: Record<LookupError["kind"], string> = {
  missingApiKey: "The dictionary isn't configured.",
  unauthorized: "The API key was rejected.",
  rateLimited: "The dictionary is busy. Try again in a moment.",
  timeout: "The dictionary took too long to respond.",
  network: "No connection to the dictionary.",
  badResponse: "The dictionary returned an unexpected response.",
  server: "The dictionary service hit an error.",
  invalidInput: "Type exactly one word to look it up.",
};

type DictionaryProps = {
  query: string;
  enabled: boolean;
  resetKey: number;
};

export function Dictionary({ query, enabled, resetKey }: DictionaryProps) {
  const { isLoading, status, entry, error } = useDictionary(
    query,
    enabled,
    resetKey,
  );

  if (isLoading && query) {
    return (
      <div className="dictionary dictionary--status" role="status">
        <span className="dictionary__spinner" aria-hidden="true" />
        <span>Looking up “{query}”…</span>
      </div>
    );
  }

  if (status === "error" && error && query) {
    return (
      <div className="dictionary dictionary--error" role="alert">
        <p className="dictionary__error-title">Couldn’t look up “{query}”.</p>
        <p className="dictionary__error-detail">
          {ERROR_MESSAGES[error.kind] ?? "Lookup failed."}
        </p>
      </div>
    );
  }

  if (!entry) return null;

  return <Entry entry={entry} />;
}

function Entry({ entry }: { entry: DictionaryEntry }) {
  return (
    <article className="dictionary">
      <div className="dictionary__heading">
        <h2 className="dictionary__word">{entry.word}</h2>
        <span className="dictionary__pos">{entry.partOfSpeech}</span>
      </div>
      <p className="dictionary__pronunciation">{entry.pronunciation}</p>
      <p className="dictionary__definition">{entry.definition}</p>
      <p className="dictionary__example">{entry.example}</p>
      <p className="dictionary__meta">
        <span>Origin: {entry.origin}</span>
        <span>Synonyms: {entry.synonyms.join(", ")}</span>
      </p>
    </article>
  );
}
