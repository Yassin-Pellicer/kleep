import type { ServiceError } from "./error";

/// The entry shape returned by the `lookup_word` command.
export type DictionaryEntry = {
  word: string;
  pronunciation: string;
  partOfSpeech: string;
  definition: string;
  example: string;
  origin: string;
  synonyms: string[];
};

export type LookupError = ServiceError;

export type DictionaryStatus = "idle" | "loading" | "found" | "error";
