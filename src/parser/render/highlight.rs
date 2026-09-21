use arborium::advanced::{Span, spans_to_html};
use arborium::{GrammarStore, Highlighter, HtmlFormat};
use std::sync::{Arc, LazyLock};

// Grammars are immutable once compiled; each renderer still owns its parse context.
static GRAMMARS: LazyLock<Arc<GrammarStore>> = LazyLock::new(|| Arc::new(GrammarStore::new()));

pub(super) fn highlighter() -> Highlighter {
    Highlighter::with_store(Arc::clone(&GRAMMARS))
}

pub(super) fn highlight_lines(code: &str, mut spans: Vec<Span>) -> String {
    spans.sort_by_key(|span| span.start);
    let mut cursor = 0usize;
    wrap_lines(code, |line, line_start, out| {
        let line_end = line_start + line.len() as u32;
        while cursor < spans.len() && spans[cursor].end <= line_start {
            cursor += 1;
        }

        let clipped = spans[cursor..]
            .iter()
            .take_while(|span| span.start < line_end)
            .filter(|span| span.end > line_start)
            .map(|span| Span {
                start: span.start.max(line_start) - line_start,
                end: span.end.min(line_end) - line_start,
                capture: span.capture.clone(),
                pattern_index: span.pattern_index,
            })
            .collect();
        out.push_str(&spans_to_html(line, clipped, &HtmlFormat::CustomElements));
    })
}

pub(super) fn wrap_plain_lines(text: &str) -> String {
    wrap_lines(text, |line, _, out| out.push_str(line))
}

fn wrap_lines(source: &str, mut body: impl FnMut(&str, u32, &mut String)) -> String {
    if source.is_empty() {
        return String::new();
    }
    let mut out = String::with_capacity(source.len() + source.len() / 4);
    let mut line_start = 0u32;
    for (i, line) in source.split('\n').enumerate() {
        if i > 0 {
            out.push('\n');
        }
        out.push_str(r#"<span class="line">"#);
        body(line, line_start, &mut out);
        out.push_str("</span>");
        line_start += line.len() as u32 + 1;
    }
    out
}
