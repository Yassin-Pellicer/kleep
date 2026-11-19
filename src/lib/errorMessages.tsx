import type { ErrorKind } from "../types";

type Overrides = Partial<Record<ErrorKind, string>>;

/// Builds the per-feature error table, keeping every wording consistent.
export function errorMessages(
  subject: string,
  overrides: Overrides = {},
): Record<ErrorKind, string> {
  return {
    missingApiKey: `The ${subject} isn't configured.`,
    unauthorized: "The API key was rejected.",
    rateLimited: `The ${subject} is busy. Try again in a moment.`,
    timeout: `The ${subject} took too long to respond.`,
    network: `No connection to the ${subject}.`,
    badResponse: `The ${subject} returned an unexpected response.`,
    server: `The ${subject} service hit an error.`,
    invalidInput: `That ${subject} request couldn't be sent.`,
    ...overrides,
  };
}
