import type { VoiceStatus } from "../../types";
import "./VoiceButton.css";

type VoiceButtonProps = {
  status: VoiceStatus;
  onClick: () => void;
  disabled?: boolean;
};

function MicIcon() {
  return (
    <svg
      width="18"
      height="18"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <path d="M12 19v3" />
      <path d="M8 22h8" />
      <rect x="9" y="2" width="6" height="12" rx="3" />
      <path d="M5 10a7 7 0 0 0 14 0" />
    </svg>
  );
}

function StopIcon() {
  return (
    <svg width="14" height="14" viewBox="0 0 14 14" aria-hidden="true">
      <rect x="2" y="2" width="10" height="10" rx="2" fill="currentColor" />
    </svg>
  );
}

export function VoiceButton({ status, onClick, disabled }: VoiceButtonProps) {
  const label =
    status === "recording"
      ? "Stop recording (stops automatically when you pause)"
      : status === "transcribing"
        ? "Transcribing"
        : status === "starting"
          ? "Starting voice input"
          : "Start voice input";

  return (
    <button
      type="button"
      className={`voice-button voice-button--${status}`}
      onClick={onClick}
      disabled={disabled || status === "starting" || status === "transcribing"}
      aria-label={label}
      title={label}
    >
      {status === "recording" ? (
        <StopIcon />
      ) : status === "starting" || status === "transcribing" ? (
        <span className="voice-button__spinner" aria-hidden="true" />
      ) : (
        <MicIcon />
      )}
    </button>
  );
}
