import React from "react";
import { useTranslation } from "react-i18next";
import type { ModelInfo } from "@/bindings";
import {
  getTranslatedModelName,
  getTranslatedModelDescription,
} from "../../lib/utils/modelTranslation";
import {
  ELEVENLABS_SCRIBE_MODEL_NAME,
  ELEVENLABS_TRANSCRIPTION_PROVIDER_ID,
} from "@/lib/utils/transcriptionProvider";

interface ModelDropdownProps {
  models: ModelInfo[];
  currentModelId: string;
  transcriptionProviderId: string;
  isScribeConfigured: boolean;
  onModelSelect: (modelId: string) => void;
  onScribeSelect: () => void;
}

const ModelDropdown: React.FC<ModelDropdownProps> = ({
  models,
  currentModelId,
  transcriptionProviderId,
  isScribeConfigured,
  onModelSelect,
  onScribeSelect,
}) => {
  const { t } = useTranslation();
  const downloadedModels = models.filter((m) => m.is_downloaded);

  const handleModelClick = (modelId: string) => {
    onModelSelect(modelId);
  };

  return (
    <div className="absolute bottom-full start-0 mb-2 w-64 max-h-[60vh] overflow-y-auto bg-background border border-mid-gray/20 rounded-lg shadow-lg py-2 z-50">
      <div
        onClick={() => isScribeConfigured && onScribeSelect()}
        onKeyDown={(event) => {
          if (
            isScribeConfigured &&
            (event.key === "Enter" || event.key === " ")
          ) {
            event.preventDefault();
            onScribeSelect();
          }
        }}
        tabIndex={isScribeConfigured ? 0 : -1}
        role="button"
        aria-disabled={!isScribeConfigured}
        className={`w-full px-3 py-2 text-start transition-colors focus:outline-none border-b border-mid-gray/20 ${
          transcriptionProviderId === ELEVENLABS_TRANSCRIPTION_PROVIDER_ID
            ? "bg-logo-primary/10 text-logo-primary"
            : ""
        } ${
          isScribeConfigured
            ? "hover:bg-mid-gray/10 cursor-pointer"
            : "opacity-50 cursor-not-allowed"
        }`}
      >
        <div className="flex items-center justify-between gap-2">
          <div className="min-w-0">
            <div className="text-sm text-text/80 truncate">
              {ELEVENLABS_SCRIBE_MODEL_NAME}
            </div>
            <div className="text-xs text-text/40 italic">
              {isScribeConfigured
                ? t("settings.scribeProvider.cloudDescription")
                : t("settings.scribeProvider.configureInModels")}
            </div>
          </div>
          {transcriptionProviderId === ELEVENLABS_TRANSCRIPTION_PROVIDER_ID && (
            <div className="text-xs text-logo-primary">
              {t("modelSelector.active")}
            </div>
          )}
        </div>
      </div>
      {downloadedModels.length > 0 ? (
        <div>
          {downloadedModels.map((model) => (
            <div
              key={model.id}
              onClick={() => handleModelClick(model.id)}
              onKeyDown={(e) => {
                if (e.key === "Enter" || e.key === " ") {
                  e.preventDefault();
                  handleModelClick(model.id);
                }
              }}
              tabIndex={0}
              role="button"
              className={`w-full px-3 py-2 text-start hover:bg-mid-gray/10 transition-colors cursor-pointer focus:outline-none ${
                transcriptionProviderId !==
                  ELEVENLABS_TRANSCRIPTION_PROVIDER_ID &&
                currentModelId === model.id
                  ? "bg-logo-primary/10 text-logo-primary"
                  : ""
              }`}
            >
              <div className="flex items-center justify-between">
                <div>
                  <div className="text-sm text-text/80">
                    {getTranslatedModelName(model, t)}
                    {model.is_custom && (
                      <span className="ms-1.5 text-[10px] font-medium text-text/40 uppercase">
                        {t("modelSelector.custom")}
                      </span>
                    )}
                    {model.supports_streaming && (
                      <span className="ms-1.5 text-[10px] font-medium text-logo-primary/70 uppercase">
                        {t("modelSelector.streaming")}
                      </span>
                    )}
                  </div>
                  <div className="text-xs text-text/40 italic pe-4">
                    {getTranslatedModelDescription(model, t)}
                  </div>
                </div>
                {transcriptionProviderId !==
                  ELEVENLABS_TRANSCRIPTION_PROVIDER_ID &&
                  currentModelId === model.id && (
                    <div className="text-xs text-logo-primary">
                      {t("modelSelector.active")}
                    </div>
                  )}
              </div>
            </div>
          ))}
        </div>
      ) : (
        <div className="px-3 py-2 text-sm text-text/60">
          {t("modelSelector.noModelsAvailable")}
        </div>
      )}
    </div>
  );
};

export default ModelDropdown;
