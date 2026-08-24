import React from "react";
import { useTranslation } from "react-i18next";
import { Check, Cloud, Globe, Loader2, Pencil, Trash2 } from "lucide-react";
import Badge from "@/components/ui/Badge";
import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { Input } from "@/components/ui/Input";
import { ELEVENLABS_SCRIBE_MODEL_NAME } from "@/lib/utils/transcriptionProvider";

interface ScribeProviderCardProps {
  isActive: boolean;
  isConfigured: boolean;
  isBusy: boolean;
  error: string | null;
  onActivate: () => Promise<void>;
  onSaveApiKey: (apiKey: string) => Promise<boolean>;
  onRemoveApiKey: () => Promise<void>;
}

export const ScribeProviderCard: React.FC<ScribeProviderCardProps> = ({
  isActive,
  isConfigured,
  isBusy,
  error,
  onActivate,
  onSaveApiKey,
  onRemoveApiKey,
}) => {
  const { t } = useTranslation();
  const [isEditing, setIsEditing] = React.useState(!isConfigured);
  const [draftApiKey, setDraftApiKey] = React.useState("");

  React.useEffect(() => {
    setIsEditing(!isConfigured);
  }, [isConfigured]);

  const handleSave = async () => {
    const apiKey = draftApiKey.trim();
    if (!apiKey || isBusy) return;

    const saved = await onSaveApiKey(apiKey);
    if (saved) {
      setDraftApiKey("");
      setIsEditing(false);
    }
  };

  return (
    <div
      className={`flex flex-col rounded-xl px-4 py-3 gap-3 text-left border-2 transition-all duration-200 ${
        isActive
          ? "border-logo-primary/50 bg-logo-primary/10"
          : "border-mid-gray/20"
      }`}
    >
      <div className="flex items-start justify-between gap-4">
        <div className="min-w-0">
          <div className="flex items-center gap-3 flex-wrap">
            <h3 className="text-base font-semibold text-text">
              {ELEVENLABS_SCRIBE_MODEL_NAME}
            </h3>
            {isActive && (
              <Badge variant="primary">
                <Check className="w-3 h-3 mr-1" />
                {t("modelSelector.active")}
              </Badge>
            )}
          </div>
          <p className="text-text/60 text-sm leading-relaxed">
            {t("settings.scribeProvider.description")}
          </p>
        </div>
        <Button
          type="button"
          variant="primary-soft"
          size="sm"
          disabled={!isConfigured || isActive || isBusy}
          onClick={() => void onActivate()}
        >
          {isBusy ? (
            <span className="flex items-center gap-1.5">
              <Loader2 className="w-3.5 h-3.5 animate-spin" />
              {t("settings.scribeProvider.working")}
            </span>
          ) : isActive ? (
            t("settings.scribeProvider.active")
          ) : (
            t("settings.scribeProvider.activate")
          )}
        </Button>
      </div>

      <div className="flex items-center gap-4 text-xs text-text/50">
        <span className="flex items-center gap-1">
          <Globe className="w-3.5 h-3.5" />
          {t("settings.scribeProvider.multilingual")}
        </span>
        <span className="flex items-center gap-1">
          <Cloud className="w-3.5 h-3.5" />
          {t("settings.scribeProvider.cloud")}
        </span>
      </div>

      <Alert variant="warning" className="py-3">
        {t("settings.scribeProvider.privacyDisclosure")}
      </Alert>
      <p className="text-xs text-text/55">
        {t("settings.scribeProvider.customWordsBilling")}
      </p>

      <div className="pt-3 border-t border-mid-gray/20">
        {isConfigured && !isEditing ? (
          <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
            <div>
              <p className="text-xs text-text/60">
                {t("settings.scribeProvider.apiKeyLabel")}
              </p>
              <p className="text-sm font-medium tracking-[0.18em] text-text/80">
                ••••••••••••
              </p>
            </div>
            <div className="flex items-center gap-2">
              <Button
                type="button"
                variant="ghost"
                size="sm"
                disabled={isBusy}
                className="flex items-center gap-1.5"
                onClick={() => setIsEditing(true)}
              >
                <Pencil className="w-3.5 h-3.5" />
                {t("settings.scribeProvider.edit")}
              </Button>
              <Button
                type="button"
                variant="danger-ghost"
                size="sm"
                disabled={isBusy}
                className="flex items-center gap-1.5"
                onClick={() => void onRemoveApiKey()}
              >
                <Trash2 className="w-3.5 h-3.5" />
                {t("settings.scribeProvider.remove")}
              </Button>
            </div>
          </div>
        ) : (
          <div className="space-y-2">
            <label
              htmlFor="elevenlabs-transcription-api-key"
              className="text-xs text-text/60"
            >
              {t("settings.scribeProvider.apiKeyLabel")}
            </label>
            <div className="flex flex-col gap-2 sm:flex-row sm:items-center">
              <Input
                id="elevenlabs-transcription-api-key"
                type="password"
                autoComplete="off"
                value={draftApiKey}
                onChange={(event) => setDraftApiKey(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === "Enter") {
                    event.preventDefault();
                    void handleSave();
                  }
                }}
                disabled={isBusy}
                placeholder={t("settings.scribeProvider.apiKeyPlaceholder")}
                className="flex-1 min-w-0 sm:min-w-[320px]"
              />
              <div className="flex items-center gap-2">
                <Button
                  type="button"
                  variant="primary-soft"
                  size="sm"
                  disabled={!draftApiKey.trim() || isBusy}
                  onClick={() => void handleSave()}
                >
                  {isBusy
                    ? t("settings.scribeProvider.saving")
                    : t("settings.scribeProvider.save")}
                </Button>
                {isConfigured && (
                  <Button
                    type="button"
                    variant="ghost"
                    size="sm"
                    disabled={isBusy}
                    onClick={() => {
                      setDraftApiKey("");
                      setIsEditing(false);
                    }}
                  >
                    {t("settings.scribeProvider.cancel")}
                  </Button>
                )}
              </div>
            </div>
          </div>
        )}
        {error && <p className="mt-2 text-xs text-red-400">{error}</p>}
        {!isConfigured && (
          <p className="mt-2 text-xs text-text/55">
            {t("settings.scribeProvider.keyRequired")}
          </p>
        )}
      </div>
    </div>
  );
};
