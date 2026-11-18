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

export type LookupErrorKind =
  | "missingApiKey"
  | "unauthorized"
  | "rateLimited"
  | "timeout"
  | "network"
  | "badResponse"
  | "server"
  | "invalidInput";

export type LookupError = {
  kind: LookupErrorKind;
  message?: string;
};

export type DictionaryStatus = "idle" | "loading" | "found" | "error";
