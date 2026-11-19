/// Error kinds shared by every service-backed feature.
export type ErrorKind =
  | "missingApiKey"
  | "unauthorized"
  | "rateLimited"
  | "timeout"
  | "network"
  | "badResponse"
  | "server"
  | "invalidInput";

/// The error shape returned by the Rust commands.
export type ServiceError = {
  kind: ErrorKind;
  message?: string;
};
