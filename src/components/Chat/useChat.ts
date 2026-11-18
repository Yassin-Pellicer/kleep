import { useEffect, useRef } from "react";
import type { AssistantStatus, ChatMessage } from "../../types";

export function useChat(messages: ChatMessage[], status: AssistantStatus) {
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const element = scrollRef.current;
    if (!element) return;
    element.scrollTo({ top: element.scrollHeight, behavior: "smooth" });
  }, [messages, status]);

  return { scrollRef };
}
