mod ast;
mod diagnostics;
mod document;
mod embed;
mod metadata;
mod render;
mod types;
mod utils;

pub use embed::EmbedParseError;
pub use metadata::extract_metadata;
pub use render::{Rendered, render, segments_html};
pub use types::{
    ContainerTag, DataAttributes, DataFilter, EmbedComponent, OutputMode, Segment, TocEntry,
};

use arborium::theme::builtin;
use napi::bindgen_prelude::*;
use napi_derive::napi;
use serde_json::{Map, Value};
use utils::into_slug;

const PARSER_STACK_SIZE: usize = 32 * 1024 * 1024;

#[napi(object)]
pub struct MogParseResult {
    pub metadata: Map<String, Value>,
    /// The document body in order — see [`Segment`].
    pub segments: Vec<Segment>,
    pub toc: Vec<TocEntry>,
    pub embed_components: Vec<EmbedComponent>,
    pub embed_css: String,
    /// Non-fatal warnings from rendering (skipped/altered content), for the
    /// host to surface — stderr is invisible in a Vite worker.
    pub diagnostics: Option<Vec<String>>,
}

#[napi(object)]
pub struct MogMetadataResult {
    pub metadata: Map<String, Value>,
    pub toc: Vec<TocEntry>,
    /// Document and metadata warnings; output-specific warnings require rendering.
    pub diagnostics: Vec<String>,
}

pub struct MetadataTask(String);

impl Task for MetadataTask {
    type Output = MogMetadataResult;
    type JsValue = MogMetadataResult;

    fn compute(&mut self) -> Result<Self::Output> {
        parse_metadata_on_bounded_stack(&self.0).map_err(Error::from_reason)
    }

    fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
        Ok(output)
    }
}

/// Metadata and heading outline without HTML rendering or syntax highlighting.
/// Embed declarations are validated, but their framework bodies are not rendered.
#[napi(ts_return_type = "Promise<MogMetadataResult>")]
pub fn parse_mog_metadata(content: String) -> AsyncTask<MetadataTask> {
    AsyncTask::new(MetadataTask(content))
}

pub fn parse_metadata_on_bounded_stack(
    content: &str,
) -> std::result::Result<MogMetadataResult, String> {
    on_bounded_stack(|| {
        let document = mog_parser::parse(content);
        let ((toc, metadata), diagnostics) = diagnostics::capture(|| {
            document::check(&document.body);
            (
                document::outline(&document.body),
                extract_metadata(document.attributes.as_deref()),
            )
        });
        let toc = toc.map_err(|err| format!("{err}. Offending line: {}", err.offending_line()))?;
        Ok(MogMetadataResult {
            metadata,
            toc,
            diagnostics,
        })
    })
}

/// Runs a parse on a dedicated, bounded stack: inline resolution recurses per
/// nesting level, and ordinary deep input must not abort the host process.
pub fn on_bounded_stack<T: Send>(
    parse: impl FnOnce() -> std::result::Result<T, String> + Send,
) -> std::result::Result<T, String> {
    std::thread::scope(|scope| {
        let handle = std::thread::Builder::new()
            .name("mog-parse".into())
            .stack_size(PARSER_STACK_SIZE)
            .spawn_scoped(scope, parse)
            .map_err(|error| format!("Failed to spawn parser thread: {error}"))?;

        handle
            .join()
            .unwrap_or_else(|_| Err("Parser thread panicked".to_string()))
    })
}

pub struct ParseTask {
    content: String,
    mode: Option<OutputMode>,
    data: DataFilter,
}

impl Task for ParseTask {
    type Output = MogParseResult;
    type JsValue = MogParseResult;

    fn compute(&mut self) -> Result<Self::Output> {
        parse_on_bounded_stack(&self.content, self.mode, self.data.clone())
            .map_err(Error::from_reason)
    }

    fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
        Ok(output)
    }
}

/// Runs on the libuv pool rather than the JS thread — Vite transforms modules
/// concurrently, and a blocking parse would serialise all of them.
///
/// The mode is typed by name, so a caller writes `'svelte'` and needs no enum import.
///
/// `dataAttributes` selects which node `` ``attr: `` keys render as `data-*`.
/// Omitted, every key renders — the plugin makes the narrower choice for its
/// own option, but the binding stays a faithful renderer.
#[napi(
    ts_args_type = "content: string, mode?: `${OutputMode}` | null, dataAttributes?: DataAttributes | null",
    ts_return_type = "Promise<MogParseResult>"
)]
pub fn parse_mog(
    content: String,
    mode: Option<OutputMode>,
    data_attributes: Option<DataAttributes>,
) -> AsyncTask<ParseTask> {
    AsyncTask::new(ParseTask {
        content,
        mode,
        data: data_attributes.into(),
    })
}

pub fn parse_on_bounded_stack(
    content: &str,
    output_mode: Option<OutputMode>,
    data: DataFilter,
) -> std::result::Result<MogParseResult, String> {
    on_bounded_stack(|| {
        let document = mog_parser::parse(content);

        // Metadata extraction warns too, so it has to run inside the capture —
        // stderr is invisible in a Vite worker.
        let ((rendered, metadata), diagnostics) = diagnostics::capture(|| {
            (
                render(&document, output_mode, data),
                extract_metadata(document.attributes.as_deref()),
            )
        });
        let rendered =
            rendered.map_err(|err| format!("{err}. Offending line: {}", err.offending_line()))?;

        Ok(MogParseResult {
            metadata,
            segments: rendered.segments,
            toc: rendered.toc,
            embed_components: rendered.embeds,
            embed_css: rendered.css,
            diagnostics: Some(diagnostics),
        })
    })
}

#[napi(object)]
#[derive(Default)]
pub struct AstOptions {
    /// Keep `` ``attr: `` blocks in the tree as `attributes` nodes instead of
    /// folding them into their owner. Default is the folded tree.
    pub unfolded: Option<bool>,
    /// Add a `plain` projection beside each `children`, by the same rule and the
    /// same code `metadata` uses. Off by default: it roughly doubles the
    /// attribute payload. Projection is silent; `parseMogMetadata` reports
    /// diagnostics about repeated keys.
    pub plain: Option<bool>,
}

pub struct AstTask {
    content: String,
    unfolded: bool,
    plain: bool,
}

impl Task for AstTask {
    type Output = String;
    type JsValue = String;

    fn compute(&mut self) -> Result<Self::Output> {
        parse_ast_json(&self.content, self.unfolded, self.plain).map_err(Error::from_reason)
    }

    fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
        Ok(output)
    }
}

/// The tree as JSON, on the same bounded stack a render uses.
pub fn parse_ast_json(
    content: &str,
    unfolded: bool,
    plain: bool,
) -> std::result::Result<String, String> {
    on_bounded_stack(|| {
        let document = match unfolded {
            true => mog_parser::parse_unfolded(content),
            false => mog_parser::parse(content),
        };

        // Projection is silent: the structured entries retain every value.
        let json = diagnostics::silence(|| serde_json::to_string(&ast::document(&document, plain)));

        json.map_err(|error| format!("Failed to serialise the document: {error}"))
    })
}

/// The document as JSON, for callers that want the tree rather than HTML.
///
/// It crosses the boundary as a string on purpose: building the object graph
/// through napi would mean several calls per node, on the JS thread, which is
/// both slower than `JSON.parse` and serialises every concurrent parse. The
/// `vite-plugin-mog/parser` entry parses it and is the typed, public form —
/// callers should use `parseMogAst` there rather than this.
#[napi(
    ts_args_type = "content: string, options?: AstOptions | null",
    ts_return_type = "Promise<string>"
)]
pub fn parse_mog_ast_json(content: String, options: Option<AstOptions>) -> AsyncTask<AstTask> {
    let options = options.unwrap_or_default();

    AsyncTask::new(AstTask {
        content,
        unfolded: options.unfolded.unwrap_or(false),
        plain: options.plain.unwrap_or(false),
    })
}

/// This crate's version, for the message when the loaded binary does not match
/// the JavaScript beside it.
#[napi]
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// A digest of the Rust sources and the dependency lock this binary was built
/// from. The JavaScript build computes the same digest over the same files, so
/// a mismatch means the two halves were built from different trees — which a
/// version cannot detect, because a local checkout and the published release it
/// shadows carry the same version string.
#[napi]
pub fn build_id() -> &'static str {
    env!("MOG_BUILD_ID")
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
}
