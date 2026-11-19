import type { ServiceError } from "../types";

/// Turns an unknown rejection into the error shape commands return.
export function normalizeError(error: unknown): ServiceError {
  if (
    typeof error === "object" &&
    error !== null &&
    "kind" in error &&
    typeof (error as { kind: unknown }).kind === "string"
  ) {
    return error as ServiceError;
  }

  return { kind: "network", message: String(error) };
}
