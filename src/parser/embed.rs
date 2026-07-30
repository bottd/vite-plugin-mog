use crate::types::OutputMode;

pub enum Embedded {
    Css(String),
    Component { mode: String, code: String },
}

/// A `` ``embed:<lang>: `` verbatim block. `css` is collected as document CSS;
/// every other language must match the plugin's output mode.
pub fn embed(
    lang: Option<&str>,
    code: &str,
    mode: Option<OutputMode>,
    index: usize,
) -> Result<Option<Embedded>, EmbedParseError> {
    let Some(lang) = lang.filter(|lang| !lang.is_empty()) else {
        return Err(EmbedParseError::MissingLanguage { index });
    };

    if lang == "css" {
        return Ok(Some(Embedded::Css(code.to_string())));
    }

    let embed_mode = lang
        .parse::<OutputMode>()
        .map_err(|_| EmbedParseError::InvalidLanguage {
            index,
            language: lang.to_string(),
        })?;

    match mode {
        None => Ok(None),
        Some(mode) if mode != embed_mode => Err(EmbedParseError::LanguageMismatch {
            index,
            language: lang.to_string(),
            mode,
        }),
        Some(_) => Ok(Some(Embedded::Component {
            mode: embed_mode.to_string(),
            code: code.to_string(),
        })),
    }
}

#[derive(Debug)]
pub enum EmbedParseError {
    MissingLanguage {
        index: usize,
    },
    InvalidLanguage {
        index: usize,
        language: String,
    },
    LanguageMismatch {
        index: usize,
        language: String,
        mode: OutputMode,
    },
}

impl EmbedParseError {
    /// The zero-based ordinal of the offending embed.
    pub fn index(&self) -> usize {
        match self {
            Self::MissingLanguage { index }
            | Self::InvalidLanguage { index, .. }
            | Self::LanguageMismatch { index, .. } => *index,
        }
    }

    /// The offending declaration, rebuilt from the parsed language rather than
    /// the source line — re-scanning the source by ordinal could point at an
    /// `embed` line sitting inside another verbatim block's raw content.
    pub fn offending_line(&self) -> String {
        match self {
            Self::MissingLanguage { .. } => "``embed:".to_string(),
            Self::InvalidLanguage { language, .. } | Self::LanguageMismatch { language, .. } => {
                format!("``embed:{language}:")
            }
        }
    }
}

impl std::fmt::Display for EmbedParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let supported = OutputMode::ALL.map(|m| m.as_str()).join(", ");
        let n = self.index() + 1;
        match self {
            Self::MissingLanguage { .. } => write!(
                f,
                "Embed error (embed #{n}): missing language. Supported languages: {supported}"
            ),
            Self::InvalidLanguage { language, .. } => write!(
                f,
                "Embed error (embed #{n}): invalid language \"{language}\". Supported languages: {supported}"
            ),
            Self::LanguageMismatch { language, mode, .. } => write!(
                f,
                "Embed error (embed #{n}): ``embed:{language}: cannot be used in {mode} mode"
            ),
        }
    }
}

impl std::error::Error for EmbedParseError {}
