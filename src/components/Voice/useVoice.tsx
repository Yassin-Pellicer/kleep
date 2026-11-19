import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type {
  VoiceRecordingStatus,
  VoiceSetupStatus,
  VoiceStatus,
  VoiceTranscript,
} from "../../types";
import { useRequestGuard } from "../../hooks/useRequestGuard";

function startVoiceRecording(): Promise<void> {
  return invoke<void>("voice_start_recording");
}

function stopVoiceRecording(): Promise<VoiceTranscript> {
  return invoke<VoiceTranscript>("voice_stop_recording");
}

function getVoiceRecordingStatus(): Promise<VoiceRecordingStatus> {
  return invoke<VoiceRecordingStatus>("voice_recording_status");
}

function getVoiceSetupStatus(): Promise<VoiceSetupStatus> {
  return invoke<VoiceSetupStatus>("voice_setup_status");
}

function cancelVoiceRecording(): Promise<void> {
  return invoke<void>("voice_cancel_recording");
}

function speakText(text: string, language: string): Promise<void> {
  return invoke<void>("voice_speak", { text, language });
}

function stopSpeaking(): Promise<void> {
  return invoke<void>("voice_stop_speaking");
}

export function useVoice() {
  const [status, setStatus] = useState<VoiceStatus>("idle");
  const [level, setLevel] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const [autoTranscript, setAutoTranscript] =
    useState<VoiceTranscript | null>(null);
  const busy = useRef(false);
  const guard = useRequestGuard();
  const pollTimer = useRef<number | null>(null);

  const stopPolling = () => {
    if (pollTimer.current !== null) {
      window.clearInterval(pollTimer.current);
      pollTimer.current = null;
      setLevel(0);
    }
  };

  useEffect(() => stopPolling, []);

  const finishAutomatically = async (id: number) => {
    stopPolling();
    setStatus("transcribing");

    try {
      const transcript = await stopVoiceRecording();
      if (!guard.isCurrent(id)) return;

      setStatus("idle");
      if (transcript.text.trim()) {
        setAutoTranscript(transcript);
      } else {
        setError("I didn't catch that. Try again.");
      }
    } catch (voiceError) {
      if (guard.isCurrent(id)) {
        setError(String(voiceError));
        setStatus("idle");
      }
    }
  };

  const startPolling = (id: number) => {
    stopPolling();
    let inFlight = false;

    pollTimer.current = window.setInterval(() => {
      if (inFlight) return;
      inFlight = true;

      void (async () => {
        if (!guard.isCurrent(id)) return;

        try {
          const state = await getVoiceRecordingStatus();
          if (!guard.isCurrent(id)) return;

          if (!state.recording && !state.autoStop) {
            stopPolling();
            setStatus("idle");
            setError("Recording stopped unexpectedly.");
            return;
          }

          setLevel(Number.isFinite(state.level) ? state.level : 0);

          if (state.autoStop) {
            await finishAutomatically(id);
          }
        } catch (voiceError) {
          if (!guard.isCurrent(id)) return;
          stopPolling();
          setError(String(voiceError));
          setStatus("idle");
          void cancelVoiceRecording().catch(() => {});
        } finally {
          inFlight = false;
        }
      })();
    }, 40);
  };

  const start = async () => {
    if (busy.current || status !== "idle") return;

    busy.current = true;
    const id = guard.next();
    setError(null);
    setAutoTranscript(null);
    setLevel(0);
    setStatus("starting");

    let setupTimer: number | null = null;
    const stopSetupPolling = () => {
      if (setupTimer !== null) {
        window.clearInterval(setupTimer);
        setupTimer = null;
      }
    };

    try {
      await stopSpeaking().catch(() => {});
      if (!guard.isCurrent(id)) return;

      // The first run provisions the Python service; show what is happening
      // instead of an indefinite spinner.
      setupTimer = window.setInterval(() => {
        void getVoiceSetupStatus()
          .then((setup) => {
            if (!guard.isCurrent(id)) return;
            if (setup.state === "preparing" && setup.message) {
              setError(setup.message);
            }
          })
          .catch(() => {});
      }, 1000);

      await startVoiceRecording();
      stopSetupPolling();
      if (!guard.isCurrent(id)) {
        await cancelVoiceRecording().catch(() => {});
        return;
      }

      setError(null);
      setStatus("recording");
      startPolling(id);
    } catch (voiceError) {
      if (guard.isCurrent(id)) {
        setError(String(voiceError));
        setStatus("idle");
      }
    } finally {
      stopSetupPolling();
      busy.current = false;
    }
  };

  const stop = async (): Promise<VoiceTranscript | null> => {
    if (status !== "recording") return null;

    const id = guard.next();
    stopPolling();
    busy.current = true;
    setStatus("transcribing");

    try {
      const transcript = await stopVoiceRecording();
      if (!guard.isCurrent(id)) return null;
      setStatus("idle");
      return transcript;
    } catch (voiceError) {
      if (guard.isCurrent(id)) {
        setError(String(voiceError));
        setStatus("idle");
      }
      return null;
    } finally {
      if (guard.isCurrent(id)) {
        busy.current = false;
      }
    }
  };

  const cancel = async () => {
    guard.invalidate();
    stopPolling();
    busy.current = false;
    setStatus("idle");
    setAutoTranscript(null);
    try {
      await cancelVoiceRecording();
    } catch {
      // Nothing was recording.
    }
  };

  const speak = async (text: string, language: string) => {
    try {
      await speakText(text, language);
    } catch (voiceError) {
      setError(String(voiceError));
    }
  };

  const stopSpeakingNow = async () => {
    try {
      await stopSpeaking();
    } catch {
      // Nothing was playing.
    }
  };

  return {
    status,
    level,
    error,
    setError,
    autoTranscript,
    clearAutoTranscript: () => setAutoTranscript(null),
    start,
    stop,
    cancel,
    speak,
    stopSpeaking: stopSpeakingNow,
  };
}
