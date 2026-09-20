use napi_derive::napi;
use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};

#[napi(string_enum)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub enum OutputMode {
    html,
    svelte,
    vue,
    react,
}

impl OutputMode {
    pub const ALL: [Self; 4] = [Self::html, Self::svelte, Self::vue, Self::react];

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::html => "html",
            Self::svelte => "svelte",
            Self::vue => "vue",
            Self::react => "react",
        }
    }
}

impl FromStr for OutputMode {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::ALL.into_iter().find(|m| m.as_str() == s).ok_or(())
    }
}

impl fmt::Display for OutputMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[napi(object)]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TocEntry {
    pub level: u32,
    pub title: String,
    pub id: String,
}

/// An embed block extracted from an `` ``embed:<mode>: `` verbatim block
#[napi(object)]
#[derive(Debug, Clone)]
pub struct EmbedComponent {
    /// Position of this component among component embeds (0-indexed)
    pub index: u32,
    /// The mode the block named, which is always the mode it was parsed in.
    pub mode: OutputMode,
    /// Raw component body from the embed block
    pub code: String,
}

/// The elements a container around an embed is lifted into.
#[napi(string_enum)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub enum ContainerTag {
    div,
    ul,
    ol,
    li,
    blockquote,
}

impl ContainerTag {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::div => "div",
            Self::ul => "ul",
            Self::ol => "ol",
            Self::li => "li",
            Self::blockquote => "blockquote",
        }
    }
}

/// One step of the rendered document, in source order.
///
/// A host framework mounts an embed as a component, which a string of HTML
/// cannot contain. So a container with an embed somewhere inside it is not
/// written into an `Html` string: it arrives as an `Open`/`Close` pair for the
/// host to build as a real element, with the embed a true descendant. Every
/// other container stays inside an `Html` segment. `Open` and `Close` always
/// balance, and an `Html` segment is never blank.
#[napi(discriminant = "kind", discriminant_case = "lowercase")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Segment {
    Html {
        html: String,
    },
    /// `index` points into the result's embed components.
    Embed {
        index: u32,
    },
    /// `classes` is the raw, space-separated class list — unescaped, possibly empty.
    Open {
        tag: ContainerTag,
        classes: String,
    },
    Close {
        tag: ContainerTag,
    },
}
