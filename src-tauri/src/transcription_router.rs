use crate::audio_toolkit::OutputLanguageEvidence;
use crate::elevenlabs_stt;
use crate::managers::transcription::{post_process_transcription_text, TranscriptionManager};
use crate::settings::{AppSettings, ELEVENLABS_TRANSCRIPTION_PROVIDER_ID};
use std::sync::Arc;

/// Transcribe audio for app-facing flows. The CLI intentionally continues to
/// call `TranscriptionManager` directly so it always remains local-only.
pub(crate) async fn transcribe_interactive(
    settings: &AppSettings,
    transcription_manager: Arc<TranscriptionManager>,
    samples: Vec<f32>,
) -> Result<String, String> {
    if settings.uses_local_transcription_provider() {
        return tauri::async_runtime::spawn_blocking(move || {
            transcription_manager
                .transcribe(samples)
                .map_err(|error| error.to_string())
        })
        .await
        .map_err(|_| "The local transcription task stopped unexpectedly.".to_string())?;
    }

    if settings.transcription_provider_id != ELEVENLABS_TRANSCRIPTION_PROVIDER_ID {
        return Err("The selected transcription provider is not supported.".to_string());
    }

    let response = elevenlabs_stt::transcribe(samples, settings)
        .await
        .map_err(|error| error.to_string())?;
    let output_language = if settings.selected_language != "auto" {
        OutputLanguageEvidence::UserSelected(normalize_language_evidence(
            &settings.selected_language,
        ))
    } else if let Some(language) = response.language_code {
        OutputLanguageEvidence::ModelDetected(normalize_language_evidence(&language))
    } else {
        OutputLanguageEvidence::Unknown
    };

    // Scribe keyterms are advisory. Run Handy's current Custom Words correction
    // as well so configured product names retain their canonical spelling
    // without carrying a private hard-coded brand dictionary in the fork.
    Ok(post_process_transcription_text(
        response.text,
        settings,
        false,
        &output_language,
        &[],
    ))
}

fn normalize_language_evidence(language: &str) -> String {
    let language = language.trim();
    if language.len() > 3 && (language.contains('-') || language.contains('_')) {
        return language.to_string();
    }
    let primary = language
        .split(&['-', '_'][..])
        .next()
        .unwrap_or(language)
        .to_ascii_lowercase();
    let normalized = if primary.len() == 3 {
        isolang::Language::from_639_3(&primary)
            .and_then(|language| language.to_639_1())
            .unwrap_or(primary.as_str())
            .to_string()
    } else if primary.len() == 2 {
        primary
    } else {
        language.to_string()
    };

    match normalized.as_str() {
        "zh" => "zh-Hans".to_string(),
        "jv" | "jav" => "jw".to_string(),
        _ => normalized,
    }
}

#[cfg(test)]
mod tests {
    use super::normalize_language_evidence;

    #[test]
    fn normalizes_provider_language_codes_for_handy_cleanup() {
        assert_eq!(normalize_language_evidence("zh"), "zh-Hans");
        assert_eq!(normalize_language_evidence("jv"), "jw");
        assert_eq!(normalize_language_evidence("eng"), "en");
        assert_eq!(normalize_language_evidence("deu"), "de");
        assert_eq!(normalize_language_evidence("zho"), "zh-Hans");
        assert_eq!(normalize_language_evidence("jav"), "jw");
        assert_eq!(normalize_language_evidence("zh-Hant"), "zh-Hant");
        assert_eq!(normalize_language_evidence("de"), "de");
    }
}
