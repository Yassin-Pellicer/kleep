export type ChatRole = "user" | "assistant";

export type ChatMessage = {
  role: ChatRole;
  content: string;
};

export type ChatSource = {
  title: string;
  url: string;
  snippet: string;
  date?: string;
};

export type AssistantReply = {
  content: string;
  sources: ChatSource[];
};

export type AssistantErrorKind =
  | "missingApiKey"
  | "unauthorized"
  | "rateLimited"
  | "timeout"
  | "network"
  | "badResponse"
  | "server"
  | "invalidInput";

export type AssistantError = {
  kind: AssistantErrorKind;
  message?: string;
};

export type AssistantStatus = "idle" | "loading" | "error";
