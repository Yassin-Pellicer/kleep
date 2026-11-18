export type VoiceStatus =
  | "idle"
  | "starting"
  | "recording"
  | "transcribing";

export type VoiceTranscript = {
  text: string;
  language: string;
};

export type VoiceRecordingStatus = {
  recording: boolean;
  autoStop: boolean;
  speech: boolean;
  silence: number;
  level: number;
};
