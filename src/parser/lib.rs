mod diagnostics;
mod embed;
mod metadata;
mod render;
mod types;
mod utils;

pub use embed::EmbedParseError;
pub use metadata::extract_metadata;
pub use render::{Rendered, render};
pub use types::{EmbedComponent, OutputMode, TocEntry};

use arborium::theme::builtin;
use napi::bindgen_prelude::*;
use napi_derive::napi;
use serde_json::{Map, Value};
use utils::into_slug;

const PARSER_STACK_SIZE: usize = 32 * 1024 * 1024;

#[napi(object)]
pub struct MogParseResult {
    pub metadata: Map<String, Value>,
    pub html_parts: Vec<String>,
    pub toc: Vec<TocEntry>,
    pub embed_components: Vec<EmbedComponent>,
    pub embed_css: String,
    /// Non-fatal warnings from rendering (skipped/altered content), for the
    /// host to surface — stderr is invisible in a Vite worker.
    pub diagnostics: Option<Vec<String>>,
}

/// Parses on a dedicated, bounded stack: inline resolution recurses per nesting
/// level, and ordinary deep input must not abort the host process.
pub fn parse_on_bounded_stack(
    content: &str,
    mode: Option<&str>,
) -> std::result::Result<MogParseResult, String> {
    std::thread::scope(|scope| {
        let handle = std::thread::Builder::new()
            .name("mog-parse".into())
            .stack_size(PARSER_STACK_SIZE)
            .spawn_scoped(scope, || parse_mog_inner(content, mode))
            .map_err(|error| format!("Failed to spawn parser thread: {error}"))?;

        handle
            .join()
            .unwrap_or_else(|_| Err("Parser thread panicked".to_string()))
    })
}

pub struct ParseTask {
    content: String,
    mode: Option<String>,
}

impl Task for ParseTask {
    type Output = MogParseResult;
    type JsValue = MogParseResult;

    fn compute(&mut self) -> Result<Self::Output> {
        parse_on_bounded_stack(&self.content, self.mode.as_deref()).map_err(Error::from_reason)
    }

    fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
        Ok(output)
    }
}

/// Runs on the libuv pool rather than the JS thread — Vite transforms modules
/// concurrently, and a blocking parse would serialise all of them.
#[napi(ts_return_type = "Promise<MogParseResult>")]
pub fn parse_mog(content: String, mode: Option<String>) -> AsyncTask<ParseTask> {
    AsyncTask::new(ParseTask { content, mode })
}

fn parse_mog_inner(
    content: &str,
    mode: Option<&str>,
) -> std::result::Result<MogParseResult, String> {
    let document = mog_parser::parse(content);
    let output_mode = mode
        .map(|mode| {
            mode.parse()
                .map_err(|()| format!("Invalid output mode: {mode}"))
        })
        .transpose()?;

    // Metadata extraction warns too, so it has to run inside the capture —
    // stderr is invisible in a Vite worker.
    let ((rendered, metadata), diagnostics) = diagnostics::capture(|| {
        (
            render(&document, output_mode),
            extract_metadata(document.meta.as_deref()),
        )
    });
    let rendered =
        rendered.map_err(|err| format!("{err}. Offending line: {}", err.offending_line()))?;

    Ok(MogParseResult {
        metadata,
        html_parts: rendered.parts,
        toc: rendered.toc,
        embed_components: rendered.embeds,
        embed_css: rendered.css,
        diagnostics: Some(diagnostics),
    })
}

/// Arborium names its themes for display — "GitHub Dark", "Solarized Light" —
/// but a Vite config reads better with a slug, so any spelling that slugs the
/// same resolves to the same theme.
#[napi]
pub fn get_theme_css(theme: String) -> String {
    let wanted = into_slug(&theme);
    builtin::all()
        .into_iter()
        .find(|t| t.name == theme || into_slug(&t.name) == wanted)
        .map(|t| t.to_css("pre.arborium"))
        .unwrap_or_default()
}

/// Every builtin theme's display name, so an unknown-theme error can show the
/// caller what it could have written.
#[napi]
pub fn theme_names() -> Vec<String> {
    builtin::all()
        .into_iter()
        .map(|t| t.name.to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn themes_resolve_by_display_name_or_slug() {
        assert!(!get_theme_css("GitHub Dark".into()).is_empty());
        assert!(!get_theme_css("github-dark".into()).is_empty());
        assert_eq!(
            get_theme_css("GitHub Dark".into()),
            get_theme_css("github-dark".into())
        );
        assert!(get_theme_css("definitely-not-a-theme".into()).is_empty());
        assert!(theme_names().contains(&"GitHub Dark".to_string()));
    }

    #[test]
    fn unknown_output_modes_are_rejected() {
        let Err(error) = parse_mog_inner("# Test", Some("metadata")) else {
            panic!("invalid mode was accepted");
        };
        assert_eq!(error, "Invalid output mode: metadata");
    }
}
