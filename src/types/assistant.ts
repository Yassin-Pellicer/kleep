import type { ServiceError } from "./error";

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

export type AssistantError = ServiceError;

export type AssistantStatus = "idle" | "loading" | "error";
