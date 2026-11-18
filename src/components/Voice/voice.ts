import { invoke } from "@tauri-apps/api/core";
import type { VoiceRecordingStatus, VoiceTranscript } from "../../types";

export function startVoiceRecording(): Promise<void> {
  return invoke<void>("voice_start_recording");
}

export function stopVoiceRecording(): Promise<VoiceTranscript> {
  return invoke<VoiceTranscript>("voice_stop_recording");
}

export function getVoiceRecordingStatus(): Promise<VoiceRecordingStatus> {
  return invoke<VoiceRecordingStatus>("voice_recording_status");
}

export function cancelVoiceRecording(): Promise<void> {
  return invoke<void>("voice_cancel_recording");
}

export function speakText(text: string, language: string): Promise<void> {
  return invoke<void>("voice_speak", { text, language });
}

export function stopSpeaking(): Promise<void> {
  return invoke<void>("voice_stop_speaking");
}
