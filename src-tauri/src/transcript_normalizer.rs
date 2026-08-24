use once_cell::sync::Lazy;
use regex::{Captures, Regex};

static EMAIL_ADDRESS: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)^[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}$").expect("valid email regex")
});
static PROTECTED_SPAN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)(?:https?://|www\.)[^\s<>()]+|`[^`]*`").expect("valid protected-span regex")
});

/// Normalize spoken forms of email addresses the user explicitly placed in
/// Custom Words. Unknown addresses, URLs, code spans, and ordinary prose are
/// deliberately left alone.
pub(crate) fn normalize_custom_word_emails(text: &str, custom_words: &[String]) -> String {
    let known_emails = custom_words
        .iter()
        .map(|word| word.trim().to_lowercase())
        .filter(|word| EMAIL_ADDRESS.is_match(word))
        .collect::<Vec<_>>();
    if known_emails.is_empty() {
        return text.to_string();
    }

    let mut output = String::with_capacity(text.len());
    let mut cursor = 0;
    for protected in PROTECTED_SPAN.find_iter(text) {
        output.push_str(&normalize_unprotected(
            &text[cursor..protected.start()],
            &known_emails,
        ));
        output.push_str(protected.as_str());
        cursor = protected.end();
    }
    output.push_str(&normalize_unprotected(&text[cursor..], &known_emails));
    output
}

fn normalize_unprotected(text: &str, known_emails: &[String]) -> String {
    let mut normalized = text.to_string();
    for email in known_emails {
        let Some(pattern) = known_email_pattern(email) else {
            continue;
        };
        loop {
            let replaced = pattern
                .replace_all(&normalized, |captures: &Captures<'_>| {
                    format!("{}{}{}", &captures[1], email, &captures[2])
                })
                .to_string();
            if replaced == normalized {
                break;
            }
            normalized = replaced;
        }
    }
    normalized
}

fn known_email_pattern(email: &str) -> Option<Regex> {
    let (local, domain) = email.split_once('@')?;
    let direct = regex::escape(email);
    let local = spoken_dotted_part(local)?;
    let domain = spoken_dotted_part(domain)?;
    let at_separator = r"(?:@|\s+(?:at(?:\s+(?:sign|zeichen))?|att|ät|klammeraffe)\s+)";

    Regex::new(&format!(
        r"(?i)(^|[^\p{{L}}\p{{N}}_])(?:{direct}|{local}{at_separator}{domain})($|[^\p{{L}}\p{{N}}_])"
    ))
    .ok()
}

fn spoken_dotted_part(part: &str) -> Option<String> {
    let tokens = part.split('.').collect::<Vec<_>>();
    if tokens.iter().any(|token| token.is_empty()) {
        return None;
    }
    Some(
        tokens
            .iter()
            .map(|token| regex::escape(token))
            .collect::<Vec<_>>()
            .join(r"(?:\.|\s+(?:dot|punkt)\s+|\s+)"),
    )
}

#[cfg(test)]
mod tests {
    use super::normalize_custom_word_emails;

    fn words() -> Vec<String> {
        vec!["alex.example@example.at".to_string()]
    }

    #[test]
    fn normalizes_only_configured_email_addresses() {
        assert_eq!(
            normalize_custom_word_emails(
                "Alex.Example@Example.AT and other.person@example.at",
                &words()
            ),
            "alex.example@example.at and other.person@example.at"
        );
    }

    #[test]
    fn normalizes_english_and_german_spoken_forms() {
        assert_eq!(
            normalize_custom_word_emails(
                "alex dot example at sign example dot at; alex punkt example klammeraffe example punkt at",
                &words()
            ),
            "alex.example@example.at; alex.example@example.at"
        );
    }

    #[test]
    fn protects_urls_and_backtick_code() {
        let input =
            "https://alex.example@example.at `Alex.Example@Example.AT` Alex.Example@Example.AT";
        assert_eq!(
            normalize_custom_word_emails(input, &words()),
            "https://alex.example@example.at `Alex.Example@Example.AT` alex.example@example.at"
        );
    }

    #[test]
    fn leaves_unrelated_words_and_unicode_adjacency_untouched() {
        let input = "Vault Orwell Geo xAlex.Example@Example.ATx";
        assert_eq!(normalize_custom_word_emails(input, &words()), input);
    }

    #[test]
    fn normalization_is_idempotent() {
        let once =
            normalize_custom_word_emails("Send to alex dot example at example dot at.", &words());
        assert_eq!(normalize_custom_word_emails(&once, &words()), once);
    }
}
