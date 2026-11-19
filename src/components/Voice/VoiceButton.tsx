import type { VoiceStatus } from "../../types";
import { MicIcon } from "../icons/MicIcon";
import { StopIcon } from "../icons/StopIcon";
import "./VoiceButton.css";

type VoiceButtonProps = {
  status: VoiceStatus;
  onClick: () => void;
  disabled?: boolean;
};

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
