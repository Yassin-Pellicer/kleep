import { useRef, useState } from "react";
import type {
  AssistantError,
  AssistantReply,
  AssistantStatus,
  ChatMessage,
} from "../../types";
import { assistantChat } from "./chat";

function normalizeError(error: unknown): AssistantError {
  if (
    typeof error === "object" &&
    error !== null &&
    "kind" in error &&
    typeof (error as { kind: unknown }).kind === "string"
  ) {
    return error as AssistantError;
  }

  return { kind: "network", message: String(error) };
}

export function useAssistant(resetKey: number) {
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [status, setStatus] = useState<AssistantStatus>("idle");
  const [error, setError] = useState<AssistantError | null>(null);
  const [sessionKey, setSessionKey] = useState(resetKey);
  const requestId = useRef(0);
  const sending = useRef(false);

  const clear = () => {
    requestId.current += 1;
    sending.current = false;
    setMessages([]);
    setStatus("idle");
    setError(null);
  };

  // Reset during render so a newly shown palette never flashes the previous
  // conversation before it collapses.
  if (sessionKey !== resetKey) {
    setSessionKey(resetKey);
    setMessages([]);
    setStatus("idle");
    setError(null);
    requestId.current += 1;
    sending.current = false;
  }

  const send = async (
    prompt: string,
    options?: { voice?: boolean },
  ): Promise<AssistantReply | null> => {
    const content = prompt.trim();
    if (!content || sending.current) return null;

    const userMessage: ChatMessage = { role: "user", content };
    const history = [...messages, userMessage];

    sending.current = true;
    setMessages(history);
    setStatus("loading");
    setError(null);

    const id = requestId.current + 1;
    requestId.current = id;

    try {
      const reply = await assistantChat(history, options?.voice ?? false);
      if (requestId.current !== id) return null;

      setMessages([
        ...history,
        ...(reply.content.trim()
          ? [{ role: "assistant" as const, content: reply.content }]
          : []),
      ]);
      setStatus("idle");
      return reply;
    } catch (lookupError: unknown) {
      if (requestId.current !== id) return null;
      setError(normalizeError(lookupError));
      setStatus("error");
      return null;
    } finally {
      if (requestId.current === id) {
        sending.current = false;
      }
    }
  };

  return { messages, status, error, send, clear };
}
