use crate::settings::{AppSettings, ELEVENLABS_DEFAULT_MODEL_ID};
use hound::{SampleFormat, WavSpec, WavWriter};
use reqwest::header::HeaderValue;
use reqwest::multipart::{Form, Part};
use reqwest::{Client, StatusCode, Url};
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashSet;
use std::fmt;
use std::io::Cursor;
use std::time::Duration;

const ELEVENLABS_SPEECH_TO_TEXT_URL: &str = "https://api.elevenlabs.io/v1/speech-to-text";
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(90);
const MIN_AUDIO_SAMPLES: usize = 1_600; // 100 ms at 16 kHz
const MAX_AUDIO_SAMPLES: usize = 16_000 * 60 * 30; // 30 minutes
const MAX_KEYTERMS: usize = 1_000;
const MAX_KEYTERM_CHARS: usize = 49;
const MAX_KEYTERM_WORDS: usize = 5;
const MAX_SUCCESS_BODY_BYTES: usize = 4 * 1024 * 1024;
const MAX_ERROR_BODY_BYTES: usize = 64 * 1024;

#[derive(Debug, PartialEq, Eq)]
pub enum ScribeError {
    MissingApiKey,
    InvalidApiKey,
    AudioTooShort,
    AudioTooLong,
    AudioEncoding,
    ClientInitialization,
    Timeout,
    Connectivity,
    RedirectRejected,
    Authentication,
    MissingPermission,
    RateLimited,
    QuotaExceeded,
    Unavailable,
    RequestRejected,
    ResponseTooLarge,
    MalformedResponse,
}

impl fmt::Display for ScribeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::MissingApiKey => "ElevenLabs API key is missing. Add it in Settings > Models.",
            Self::InvalidApiKey => "The configured ElevenLabs API key is invalid.",
            Self::AudioTooShort => "The recording is too short for ElevenLabs transcription.",
            Self::AudioTooLong => {
                "The recording exceeds Handy's 30-minute ElevenLabs upload limit."
            }
            Self::AudioEncoding => "Handy could not prepare the recording for ElevenLabs.",
            Self::ClientInitialization => "Handy could not initialize the ElevenLabs client.",
            Self::Timeout => "The ElevenLabs transcription request timed out.",
            Self::Connectivity => {
                "Handy could not reach ElevenLabs. Check the internet connection and try again."
            }
            Self::RedirectRejected => "ElevenLabs returned an unexpected redirect.",
            Self::Authentication => {
                "ElevenLabs authentication failed. Check the configured API key."
            }
            Self::MissingPermission => {
                "The ElevenLabs API key does not have speech-to-text permission."
            }
            Self::RateLimited => "ElevenLabs is rate limiting transcription requests.",
            Self::QuotaExceeded => "The ElevenLabs speech-to-text quota is exhausted.",
            Self::Unavailable => "ElevenLabs is temporarily unavailable. Please try again later.",
            Self::RequestRejected => "ElevenLabs rejected the transcription request.",
            Self::ResponseTooLarge => "ElevenLabs returned an unexpectedly large response.",
            Self::MalformedResponse => "ElevenLabs returned an invalid transcription response.",
        };
        f.write_str(message)
    }
}

impl std::error::Error for ScribeError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScribeTranscription {
    pub text: String,
    pub language_code: Option<String>,
}

#[derive(Deserialize)]
struct ScribeResponse {
    text: String,
    #[serde(default)]
    language_code: Option<String>,
}

pub async fn transcribe(
    audio: Vec<f32>,
    settings: &AppSettings,
) -> Result<ScribeTranscription, ScribeError> {
    let endpoint = resolve_endpoint()?;
    transcribe_to_endpoint(audio, settings, endpoint).await
}

async fn transcribe_to_endpoint(
    audio: Vec<f32>,
    settings: &AppSettings,
    endpoint: Url,
) -> Result<ScribeTranscription, ScribeError> {
    validate_audio_len(audio.len())?;
    let api_key = settings
        .elevenlabs_api_key()
        .ok_or(ScribeError::MissingApiKey)?;
    let mut api_key = HeaderValue::from_str(api_key).map_err(|_| ScribeError::InvalidApiKey)?;
    api_key.set_sensitive(true);
    // Encoding a long recording is CPU work. Keeping it off the async executor
    // also means dropping this future can prevent the HTTP upload from starting,
    // even though the already-running bounded conversion itself cannot abort.
    let wav_bytes = tokio::task::spawn_blocking(move || encode_wav(&audio))
        .await
        .map_err(|_| ScribeError::AudioEncoding)??;
    let audio_part = Part::bytes(wav_bytes)
        .file_name("handy-recording.wav")
        .mime_str("audio/wav")
        .map_err(|_| ScribeError::AudioEncoding)?;

    let mut form = Form::new()
        .text("model_id", ELEVENLABS_DEFAULT_MODEL_ID.to_string())
        .text("tag_audio_events", "false")
        .text("no_verbatim", "true")
        .part("file", audio_part);

    if let Some(language_code) = normalize_language_code(&settings.selected_language) {
        form = form.text("language_code", language_code);
    }
    for keyterm in collect_keyterms(&settings.custom_words) {
        form = form.text("keyterms", keyterm);
    }

    let client = Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(REQUEST_TIMEOUT)
        .build()
        .map_err(|_| ScribeError::ClientInitialization)?;

    let response = client
        .post(endpoint)
        .header("xi-api-key", api_key)
        .multipart(form)
        .send()
        .await
        .map_err(map_transport_error)?;
    let status = response.status();
    let body_limit = if status.is_success() {
        MAX_SUCCESS_BODY_BYTES
    } else {
        MAX_ERROR_BODY_BYTES
    };
    let body = read_body_limited(response, body_limit).await?;

    if status.is_redirection() {
        return Err(ScribeError::RedirectRejected);
    }
    if !status.is_success() {
        return Err(classify_provider_error(status, &body));
    }

    let payload: ScribeResponse =
        serde_json::from_slice(&body).map_err(|_| ScribeError::MalformedResponse)?;
    let text = payload.text.trim().to_string();
    Ok(ScribeTranscription {
        text,
        language_code: payload
            .language_code
            .map(|language| language.trim().to_string())
            .filter(|language| !language.is_empty()),
    })
}

fn validate_audio_len(samples: usize) -> Result<(), ScribeError> {
    if samples < MIN_AUDIO_SAMPLES {
        return Err(ScribeError::AudioTooShort);
    }
    if samples > MAX_AUDIO_SAMPLES {
        return Err(ScribeError::AudioTooLong);
    }
    Ok(())
}

fn encode_wav(audio: &[f32]) -> Result<Vec<u8>, ScribeError> {
    let mut cursor = Cursor::new(Vec::with_capacity(audio.len() * 2 + 44));
    let spec = WavSpec {
        channels: 1,
        sample_rate: 16_000,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };
    {
        let mut writer =
            WavWriter::new(&mut cursor, spec).map_err(|_| ScribeError::AudioEncoding)?;
        for sample in audio {
            let clamped = sample.clamp(-1.0, 1.0);
            writer
                .write_sample((clamped * i16::MAX as f32) as i16)
                .map_err(|_| ScribeError::AudioEncoding)?;
        }
        writer.finalize().map_err(|_| ScribeError::AudioEncoding)?;
    }
    Ok(cursor.into_inner())
}

fn normalize_language_code(language: &str) -> Option<String> {
    match language.trim() {
        "" | "auto" => None,
        "zh-Hans" | "zh-Hant" => Some("zh".to_string()),
        "jw" => Some("jv".to_string()),
        language => Some(language.to_string()),
    }
}

fn collect_keyterms(custom_words: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut keyterms = Vec::new();

    for custom_word in custom_words {
        let keyterm = custom_word.trim();
        if !valid_keyterm(keyterm) {
            continue;
        }
        let dedupe_key = keyterm.to_lowercase();
        if seen.insert(dedupe_key) {
            keyterms.push(keyterm.to_string());
            if keyterms.len() == MAX_KEYTERMS {
                break;
            }
        }
    }

    keyterms
}

fn valid_keyterm(keyterm: &str) -> bool {
    !keyterm.is_empty()
        && keyterm.chars().count() <= MAX_KEYTERM_CHARS
        && keyterm.split_whitespace().count() <= MAX_KEYTERM_WORDS
        && !keyterm
            .chars()
            .any(|character| matches!(character, '<' | '>' | '{' | '}' | '[' | ']' | '\\'))
}

fn resolve_endpoint() -> Result<Url, ScribeError> {
    Url::parse(ELEVENLABS_SPEECH_TO_TEXT_URL).map_err(|_| ScribeError::ClientInitialization)
}

fn map_transport_error(error: reqwest::Error) -> ScribeError {
    if error.is_timeout() {
        ScribeError::Timeout
    } else {
        ScribeError::Connectivity
    }
}

async fn read_body_limited(
    mut response: reqwest::Response,
    limit: usize,
) -> Result<Vec<u8>, ScribeError> {
    if response
        .content_length()
        .is_some_and(|length| length > limit as u64)
    {
        return Err(ScribeError::ResponseTooLarge);
    }

    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(map_transport_error)? {
        if body.len().saturating_add(chunk.len()) > limit {
            return Err(ScribeError::ResponseTooLarge);
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

fn classify_provider_error(status: StatusCode, body: &[u8]) -> ScribeError {
    // ElevenLabs sometimes reports permission and quota states with HTTP 401.
    // Classify only the stable machine-readable status token, never the
    // provider's free-form message or raw response body.
    match provider_status(body).as_deref() {
        Some("missing_permissions") => return ScribeError::MissingPermission,
        Some("quota_exceeded" | "credits_exhausted" | "insufficient_credits") => {
            return ScribeError::QuotaExceeded;
        }
        Some("concurrency_limit_exceeded" | "rate_limit_exceeded") => {
            return ScribeError::RateLimited;
        }
        Some("invalid_api_key" | "invalid_api_key_format") => {
            return ScribeError::InvalidApiKey;
        }
        _ => {}
    }

    if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
        return ScribeError::Authentication;
    }
    if status == StatusCode::PAYMENT_REQUIRED {
        return ScribeError::QuotaExceeded;
    }
    if status == StatusCode::TOO_MANY_REQUESTS {
        return ScribeError::RateLimited;
    }
    if status.is_server_error() {
        return ScribeError::Unavailable;
    }
    ScribeError::RequestRejected
}

fn provider_status(body: &[u8]) -> Option<String> {
    let payload: Value = serde_json::from_slice(body).ok()?;
    match payload.get("detail") {
        Some(Value::Object(detail)) => detail
            .get("status")
            .and_then(Value::as_str)
            .map(|status| status.to_lowercase()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::{get_default_settings, ELEVENLABS_TRANSCRIPTION_PROVIDER_ID};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    #[test]
    fn validates_audio_bounds_without_allocating_large_buffers() {
        assert_eq!(validate_audio_len(0), Err(ScribeError::AudioTooShort));
        assert_eq!(validate_audio_len(1_599), Err(ScribeError::AudioTooShort));
        assert_eq!(validate_audio_len(1_600), Ok(()));
        assert_eq!(validate_audio_len(MAX_AUDIO_SAMPLES), Ok(()));
        assert_eq!(
            validate_audio_len(MAX_AUDIO_SAMPLES + 1),
            Err(ScribeError::AudioTooLong)
        );
    }

    #[test]
    fn wav_encoding_is_mono_16khz_pcm() {
        let bytes = encode_wav(&[0.0; MIN_AUDIO_SAMPLES]).unwrap();
        let reader = hound::WavReader::new(Cursor::new(bytes)).unwrap();
        let spec = reader.spec();
        assert_eq!(spec.channels, 1);
        assert_eq!(spec.sample_rate, 16_000);
        assert_eq!(spec.bits_per_sample, 16);
        assert_eq!(spec.sample_format, SampleFormat::Int);
    }

    #[test]
    fn language_mapping_omits_auto_and_normalizes_known_aliases() {
        assert_eq!(normalize_language_code("auto"), None);
        assert_eq!(normalize_language_code("zh-Hant"), Some("zh".into()));
        assert_eq!(normalize_language_code("zh-Hans"), Some("zh".into()));
        assert_eq!(normalize_language_code("jw"), Some("jv".into()));
        assert_eq!(normalize_language_code("de"), Some("de".into()));
    }

    #[test]
    fn keyterms_are_trimmed_deduplicated_filtered_and_stable() {
        let words = vec![
            "  immohelfer  ".to_string(),
            "Immohelfer".to_string(),
            "one two three four five".to_string(),
            "one two three four five six".to_string(),
            "bad{term".to_string(),
            "x".repeat(50),
            "christoph@example.at".to_string(),
        ];
        assert_eq!(
            collect_keyterms(&words),
            vec![
                "immohelfer",
                "one two three four five",
                "christoph@example.at"
            ]
        );
    }

    #[test]
    fn keyterms_are_capped_at_provider_limit() {
        let words = (0..=MAX_KEYTERMS)
            .map(|index| format!("term-{index}"))
            .collect::<Vec<_>>();
        let keyterms = collect_keyterms(&words);
        assert_eq!(keyterms.len(), MAX_KEYTERMS);
        assert_eq!(keyterms.first().map(String::as_str), Some("term-0"));
        assert_eq!(keyterms.last().map(String::as_str), Some("term-999"));
    }

    #[test]
    fn provider_errors_are_classified_without_exposing_response_details() {
        let permission = br#"{"detail":{"status":"missing_permissions","message":"private"}}"#;
        assert_eq!(
            classify_provider_error(StatusCode::FORBIDDEN, permission),
            ScribeError::MissingPermission
        );
        assert_eq!(
            classify_provider_error(StatusCode::UNAUTHORIZED, permission),
            ScribeError::MissingPermission
        );
        let quota = br#"{"detail":{"status":"quota_exceeded","message":"private"}}"#;
        assert_eq!(
            classify_provider_error(StatusCode::TOO_MANY_REQUESTS, quota),
            ScribeError::QuotaExceeded
        );
        assert_eq!(
            classify_provider_error(StatusCode::UNAUTHORIZED, quota),
            ScribeError::QuotaExceeded
        );
        let invalid = br#"{"detail":{"status":"invalid_api_key","message":"private"}}"#;
        assert_eq!(
            classify_provider_error(StatusCode::UNAUTHORIZED, invalid),
            ScribeError::InvalidApiKey
        );
        assert_eq!(
            classify_provider_error(StatusCode::INTERNAL_SERVER_ERROR, b"private"),
            ScribeError::Unavailable
        );
    }

    async fn serve_once(response: &'static str) -> (Url, tokio::task::JoinHandle<Vec<u8>>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let handle = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            let mut content_length = None;
            let mut header_end = None;
            loop {
                let mut chunk = [0_u8; 8 * 1024];
                let read = stream.read(&mut chunk).await.unwrap();
                if read == 0 {
                    break;
                }
                request.extend_from_slice(&chunk[..read]);
                if header_end.is_none() {
                    header_end = request.windows(4).position(|window| window == b"\r\n\r\n");
                    if let Some(end) = header_end {
                        let headers = String::from_utf8_lossy(&request[..end]);
                        content_length = headers.lines().find_map(|line| {
                            line.strip_prefix("content-length: ")
                                .or_else(|| line.strip_prefix("Content-Length: "))
                                .and_then(|value| value.parse::<usize>().ok())
                        });
                    }
                }
                if let (Some(end), Some(length)) = (header_end, content_length) {
                    if request.len() >= end + 4 + length {
                        break;
                    }
                }
            }
            stream.write_all(response.as_bytes()).await.unwrap();
            request
        });
        (
            Url::parse(&format!("http://{address}/v1/speech-to-text")).unwrap(),
            handle,
        )
    }

    fn configured_settings() -> AppSettings {
        let mut settings = get_default_settings();
        settings.transcription_api_keys.insert(
            ELEVENLABS_TRANSCRIPTION_PROVIDER_ID.to_string(),
            "test-api-key".to_string(),
        );
        settings.selected_language = "de".to_string();
        settings.custom_words = vec!["Handy".to_string(), "Handy".to_string()];
        settings
    }

    #[tokio::test]
    async fn sends_bounded_multipart_contract_and_parses_success() {
        let response_body = r#"{"text":" Hallo Welt ","language_code":"de"}"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            response_body.len(),
            response_body
        );
        let response: &'static str = Box::leak(response.into_boxed_str());
        let (endpoint, server) = serve_once(response).await;

        let result = transcribe_to_endpoint(
            vec![0.0; MIN_AUDIO_SAMPLES],
            &configured_settings(),
            endpoint,
        )
        .await
        .unwrap();
        let request = String::from_utf8_lossy(&server.await.unwrap()).to_string();

        assert_eq!(result.text, "Hallo Welt");
        assert_eq!(result.language_code.as_deref(), Some("de"));
        assert!(request.contains("xi-api-key: test-api-key"));
        assert!(request.contains("name=\"model_id\""));
        assert!(request.contains(ELEVENLABS_DEFAULT_MODEL_ID));
        assert!(request.contains("name=\"tag_audio_events\""));
        assert!(request.contains("name=\"no_verbatim\""));
        assert!(request.contains("name=\"language_code\""));
        assert!(request.contains("name=\"tag_audio_events\"\r\n\r\nfalse"));
        assert!(request.contains("name=\"no_verbatim\"\r\n\r\ntrue"));
        assert!(request.contains("name=\"language_code\"\r\n\r\nde"));
        assert_eq!(request.matches("name=\"keyterms\"").count(), 1);
        assert!(request.contains("name=\"keyterms\"\r\n\r\nHandy"));
        assert!(request.contains("filename=\"handy-recording.wav\""));
    }

    #[tokio::test]
    async fn refuses_redirects_without_following_them() {
        let (endpoint, server) = serve_once(
            "HTTP/1.1 302 Found\r\nLocation: https://example.com/private\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        )
        .await;
        let result = transcribe_to_endpoint(
            vec![0.0; MIN_AUDIO_SAMPLES],
            &configured_settings(),
            endpoint,
        )
        .await;
        let _ = server.await.unwrap();
        assert_eq!(result, Err(ScribeError::RedirectRejected));
    }

    #[tokio::test]
    async fn omits_language_field_for_auto_detection() {
        let response_body = r#"{"text":"hello","language_code":"en"}"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            response_body.len(),
            response_body
        );
        let response: &'static str = Box::leak(response.into_boxed_str());
        let (endpoint, server) = serve_once(response).await;
        let mut settings = configured_settings();
        settings.selected_language = "auto".to_string();

        transcribe_to_endpoint(vec![0.0; MIN_AUDIO_SAMPLES], &settings, endpoint)
            .await
            .unwrap();
        let request = String::from_utf8_lossy(&server.await.unwrap()).to_string();
        assert!(!request.contains("name=\"language_code\""));
    }
}
