import { useEffect, useRef, useState } from "react";
import type { VoiceStatus, VoiceTranscript } from "../../types";
import {
  cancelVoiceRecording,
  getVoiceRecordingStatus,
  speakText,
  startVoiceRecording,
  stopSpeaking,
  stopVoiceRecording,
} from "./voice";

export function useVoice() {
  const [status, setStatus] = useState<VoiceStatus>("idle");
  const [error, setError] = useState<string | null>(null);
  const [autoTranscript, setAutoTranscript] =
    useState<VoiceTranscript | null>(null);
  const busy = useRef(false);
  const sessionId = useRef(0);
  const pollTimer = useRef<number | null>(null);

  const stopPolling = () => {
    if (pollTimer.current !== null) {
      window.clearInterval(pollTimer.current);
      pollTimer.current = null;
    }
  };

  useEffect(() => stopPolling, []);

  const finishAutomatically = async (id: number) => {
    stopPolling();
    setStatus("transcribing");

    try {
      const transcript = await stopVoiceRecording();
      if (sessionId.current !== id) return;

      setStatus("idle");
      if (transcript.text.trim()) {
        setAutoTranscript(transcript);
      } else {
        setError("I didn't catch that. Try again.");
      }
    } catch (voiceError) {
      if (sessionId.current === id) {
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
        if (sessionId.current !== id) return;

        try {
          const state = await getVoiceRecordingStatus();
          if (sessionId.current !== id) return;

          if (!state.recording && !state.autoStop) {
            stopPolling();
            setStatus("idle");
            setError("Recording stopped unexpectedly.");
            return;
          }

          if (state.autoStop) {
            await finishAutomatically(id);
          }
        } catch (voiceError) {
          if (sessionId.current !== id) return;
          stopPolling();
          setError(String(voiceError));
          setStatus("idle");
          void cancelVoiceRecording().catch(() => {});
        } finally {
          inFlight = false;
        }
      })();
    }, 150);
  };

  const start = async () => {
    if (busy.current || status !== "idle") return;

    busy.current = true;
    const id = sessionId.current + 1;
    sessionId.current = id;
    setError(null);
    setAutoTranscript(null);
    setStatus("starting");

    try {
      await stopSpeaking().catch(() => {});
      if (sessionId.current !== id) return;

      await startVoiceRecording();
      if (sessionId.current !== id) {
        await cancelVoiceRecording().catch(() => {});
        return;
      }

      setStatus("recording");
      startPolling(id);
    } catch (voiceError) {
      if (sessionId.current === id) {
        setError(String(voiceError));
        setStatus("idle");
      }
    } finally {
      busy.current = false;
    }
  };

  const stop = async (): Promise<VoiceTranscript | null> => {
    if (status !== "recording") return null;

    const id = sessionId.current + 1;
    sessionId.current = id;
    stopPolling();
    busy.current = true;
    setStatus("transcribing");

    try {
      const transcript = await stopVoiceRecording();
      if (sessionId.current !== id) return null;
      setStatus("idle");
      return transcript;
    } catch (voiceError) {
      if (sessionId.current === id) {
        setError(String(voiceError));
        setStatus("idle");
      }
      return null;
    } finally {
      if (sessionId.current === id) {
        busy.current = false;
      }
    }
  };

  const cancel = async () => {
    sessionId.current += 1;
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
