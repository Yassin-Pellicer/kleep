import { openUrl } from "@tauri-apps/plugin-opener";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import type {
  AssistantError,
  AssistantStatus,
  ChatMessage,
} from "../../types";
import { errorMessages } from "../../lib/errorMessages";
import { useChat } from "./useChat";
import { rehypeFadeWords } from "./rehypeFadeWords";
import "./Chat.css";

const ERROR_MESSAGES = errorMessages("assistant", {
  invalidInput: "That prompt couldn't be sent.",
});

type ChatProps = {
  messages: ChatMessage[];
  status: AssistantStatus;
  error: AssistantError | null;
  onExit: () => void;
};

export function Chat({ messages, status, error, onExit }: ChatProps) {
  const { scrollRef } = useChat(messages, status);

  return (
    <section className="chat" aria-label="Assistant conversation">
      <div className="chat__top">
        <button
          type="button"
          className="chat__close"
          onClick={onExit}
          aria-label="Exit chat"
          title="Exit chat (Esc)"
        >
          ×
        </button>
      </div>

      <div className="chat__messages" ref={scrollRef}>
        {messages.length === 0 && status === "idle" && (
          <p className="chat__hint">Press Enter to ask Kleep anything.</p>
        )}

        {messages.map((message, index) =>
          message.role === "user" ? (
            <div
              key={`user-${index}`}
              className="chat__user"
            >
              <p className="chat__user-text">{message.content}</p>
            </div>
          ) : (
            <div key={`assistant-${index}`} className="chat__answer">
              <ReactMarkdown
                remarkPlugins={[remarkGfm]}
                rehypePlugins={[rehypeFadeWords]}
                components={{
                  a: ({ href, children }) => (
                    <a
                      href={href}
                      onClick={(event) => {
                        event.preventDefault();
                        if (href) {
                          void openUrl(href).catch(() => { });
                        }
                      }}
                    >
                      {children}
                    </a>
                  ),
                }}
              >
                {message.content}
              </ReactMarkdown>
            </div>
          ),
        )}

        {status === "loading" && (
          <span className="chat__typing" aria-label="Thinking">
            <i />
            <i />
            <i />
          </span>
        )}

        {status === "error" && error && (
          <p className="chat__error" role="alert">
            {ERROR_MESSAGES[error.kind] ?? "Something went wrong."}
            {error.kind === "badResponse" && error.message
              ? ` — ${error.message}`
              : ""}
          </p>
        )}
      </div>
    </section>
  );
}
