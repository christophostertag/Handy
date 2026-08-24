import type { AppSettings } from "@/bindings";

export const LOCAL_TRANSCRIPTION_PROVIDER_ID = "local";
export const ELEVENLABS_TRANSCRIPTION_PROVIDER_ID = "elevenlabs";
export const ELEVENLABS_SCRIBE_MODEL_NAME = "Scribe v2 by ElevenLabs";

export const getTranscriptionProviderId = (
  settings?: AppSettings | null,
): string => {
  const providerId = settings?.transcription_provider_id;

  return providerId === ELEVENLABS_TRANSCRIPTION_PROVIDER_ID
    ? ELEVENLABS_TRANSCRIPTION_PROVIDER_ID
    : LOCAL_TRANSCRIPTION_PROVIDER_ID;
};

export const hasTranscriptionProviderApiKey = (
  settings: AppSettings | null | undefined,
  providerId: string,
): boolean => {
  const apiKey = settings?.transcription_api_keys?.[providerId];

  return typeof apiKey === "string" && apiKey.trim().length > 0;
};
