import { invoke } from "@tauri-apps/api/core";
import type { AssistantReply, ChatMessage } from "../../types";

export function assistantChat(
  messages: ChatMessage[],
  voice = false,
): Promise<AssistantReply> {
  return invoke<AssistantReply>("assistant_chat", { messages, voice });
}
