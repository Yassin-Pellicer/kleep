import { useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type {
  AssistantError,
  AssistantReply,
  AssistantStatus,
  ChatMessage,
} from "../../types";
import { useRequestGuard } from "../../hooks/useRequestGuard";
import { normalizeError } from "../../lib/normalizeError";

function assistantChat(messages: ChatMessage[]): Promise<AssistantReply> {
  return invoke<AssistantReply>("assistant_chat", { messages });
}

export function useAssistant(resetKey: number) {
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [status, setStatus] = useState<AssistantStatus>("idle");
  const [error, setError] = useState<AssistantError | null>(null);
  const [sessionKey, setSessionKey] = useState(resetKey);
  const guard = useRequestGuard();
  const sending = useRef(false);

  const clear = () => {
    guard.invalidate();
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
    guard.invalidate();
    sending.current = false;
  }

  const send = async (prompt: string): Promise<AssistantReply | null> => {
    const content = prompt.trim();
    if (!content || sending.current) return null;

    const userMessage: ChatMessage = { role: "user", content };
    const history = [...messages, userMessage];

    sending.current = true;
    setMessages(history);
    setStatus("loading");
    setError(null);

    const id = guard.next();

    try {
      const reply = await assistantChat(history);
      if (!guard.isCurrent(id)) return null;

      setMessages([
        ...history,
        ...(reply.content.trim()
          ? [{ role: "assistant" as const, content: reply.content }]
          : []),
      ]);
      setStatus("idle");
      return reply;
    } catch (lookupError: unknown) {
      if (!guard.isCurrent(id)) return null;
      setError(normalizeError(lookupError));
      setStatus("error");
      return null;
    } finally {
      if (guard.isCurrent(id)) {
        sending.current = false;
      }
    }
  };

  return { messages, status, error, send, clear };
}
