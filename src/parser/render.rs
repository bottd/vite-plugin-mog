use crate::embed::{EmbedParseError, Embedded, embed};
use crate::types::{EmbedComponent, OutputMode, TocEntry};
use crate::utils::{UrlKind, has_unsafe_scheme, into_slug};
use arborium::advanced::{Span, spans_to_html};
use arborium::{Highlighter, HtmlFormat};
use htmlescape::encode_minimal;
use mog_parser::{Data, Delimiter, Document, MarkerKind, Node, NodeKind, Value};
use std::collections::HashSet;
use std::fmt::Write;

pub struct Rendered {
    /// HTML between embeds — one more part than there are components.
    pub parts: Vec<String>,
    pub embeds: Vec<EmbedComponent>,
    pub css: String,
    pub toc: Vec<TocEntry>,
}

pub fn render(document: &Document, mode: Option<OutputMode>) -> Result<Rendered, EmbedParseError> {
    let mut renderer = Renderer::new(mode);
    renderer.blocks(&document.body)?;
    Ok(renderer.finish())
}

struct Renderer {
    parts: Vec<String>,
    out: String,
    embeds: Vec<EmbedComponent>,
    css: Vec<String>,
    toc: Vec<TocEntry>,
    /// Rendered footnote bodies, in reference order; emitted as one list at the
    /// end of the document.
    footnotes: Vec<String>,
    ids: HashSet<String>,
    mode: Option<OutputMode>,
    highlighter: Highlighter,
    /// Counts every embed declaration the renderer visits (incl. CSS, `None`
    /// mode, and failing ones), giving errors their "embed #N" number.
    embed_decls: usize,
    list_depth: usize,
}

impl Renderer {
    fn new(mode: Option<OutputMode>) -> Self {
        Self {
            parts: Vec::new(),
            out: String::new(),
            embeds: Vec::new(),
            css: Vec::new(),
            toc: Vec::new(),
            footnotes: Vec::new(),
            ids: HashSet::new(),
            mode,
            highlighter: Highlighter::new(),
            embed_decls: 0,
            list_depth: 0,
        }
    }

    fn finish(mut self) -> Rendered {
        if !self.footnotes.is_empty() {
            self.out.push_str(r#"<aside class="footnotes"><ol>"#);
            for (index, body) in self.footnotes.iter().enumerate() {
                let n = index + 1;
                let _ = write!(
                    self.out,
                    r##"<li id="footnote-{n}">{body} <a href="#footnote-ref-{n}" class="footnote-backref">&#x21a9;</a></li>"##
                );
            }
            self.out.push_str("</ol></aside>\n");
        }
        self.parts.push(self.out);
        Rendered {
            parts: self.parts,
            embeds: self.embeds,
            css: self.css.join("\n"),
            toc: self.toc,
        }
    }

    fn push_block(&mut self, html: &str) {
        self.out.push_str(html);
        self.out.push('\n');
    }

    fn blocks(&mut self, nodes: &[Node]) -> Result<(), EmbedParseError> {
        let mut index = 0;
        while index < nodes.len() {
            // Adjacent list-like markers form one run: mog emits a nested item
            // as a flat sibling with a deeper `depth`, so re-nesting needs to
            // see the whole run at once.
            let start = index;
            while index < nodes.len() && list_kind(&nodes[index]).is_some() {
                index += 1;
            }
            if index > start {
                self.list(&nodes[start..index])?;
                continue;
            }

            // Adjacent inline nodes at block level become one paragraph.
            let start = index;
            while index < nodes.len() && !is_block(&nodes[index]) {
                index += 1;
            }
            if index > start {
                self.paragraph(&nodes[start..index], "");
                continue;
            }

            self.block(&nodes[index])?;
            index += 1;
        }
        Ok(())
    }

    fn block(&mut self, node: &Node) -> Result<(), EmbedParseError> {
        match &node.kind {
            NodeKind::Marker(marker) if marker.kind == MarkerKind::Heading => {
                let (title, content) = split_content(node);
                let title_text = text_of(title);
                let title_html = self.inline(title);
                let level = marker.depth.min(6);
                let id = self.unique_id(into_slug(&title_text));
                let id_attr = match id.is_empty() {
                    // A symbol-only title slugs to "" — omit rather than emit
                    // an HTML5-invalid `id=""`.
                    true => String::new(),
                    false => format!(" id=\"{id}\""),
                };
                let (classes, status) = marker_markup(node);
                self.push_block(&format!(
                    "<h{level}{id_attr}{classes}>{status}{}{title_html}</h{level}>",
                    gap(status, &title_html)
                ));
                if !id.is_empty() {
                    self.toc.push(TocEntry {
                        level: level as u32,
                        title: title_text,
                        id,
                    });
                }
                self.blocks(content)?;
            }
            // A free marker carries no meaning of its own: it is a grouping
            // block, so it renders as one and whatever it captured renders in it.
            NodeKind::Marker(marker) if marker.kind == MarkerKind::Free => {
                self.push_block(&format!("<div{}>", class_attr(node)));
                self.blocks(children(node))?;
                self.push_block("</div>");
            }
            NodeKind::Paragraph => self.paragraph(children(node), &class_attr(node)),
            NodeKind::Table => self.table(node),
            NodeKind::Delimiter(Delimiter::Verbatim) => self.verbatim(node)?,
            // Nothing else reaches here: `blocks` takes list markers as runs and
            // inline nodes as paragraph runs before dispatching, and a `data`
            // block carries structured data for the host rather than content.
            _ => {}
        }
        Ok(())
    }

    fn paragraph(&mut self, nodes: &[Node], classes: &str) {
        let html = self.inline(nodes);
        if !html.trim().is_empty() {
            self.push_block(&format!("<p{classes}>{html}</p>"));
        }
    }

    fn list(&mut self, items: &[Node]) -> Result<(), EmbedParseError> {
        let mut stack: Vec<(MarkerKind, usize)> = Vec::new();
        self.list_depth += 1;

        for node in items {
            let Some((kind, depth)) = list_kind(node) else {
                continue;
            };

            // Each `>` is its own quote — several lines land in one quote only in
            // block form, never across sibling markers. A list does collect its
            // items, so only a blockquote closes on a same-depth sibling.
            while stack.last().is_some_and(|&(open, level)| {
                level > depth
                    || (level == depth && (open != kind || kind == MarkerKind::Blockquote))
            }) {
                let (open, _) = stack.pop().expect("open container");
                self.close_item(open);
                let _ = write!(self.out, "</{}>", container_tag(open));
            }

            let (classes, status) = marker_markup(node);
            // A marker decorates the element it produces: a list marker its
            // `<li>`, a `>` the blockquote it always opens.
            let (container_classes, item_classes) = match kind {
                MarkerKind::Blockquote => (classes.as_str(), ""),
                _ => ("", classes.as_str()),
            };
            match stack.last() {
                Some(&(_, level)) if level == depth => self.close_item(kind),
                _ => {
                    let _ = write!(self.out, "<{}{container_classes}>", container_tag(kind));
                    stack.push((kind, depth));
                }
            }

            let (inline, content) = split_content(node);
            let html = self.inline(inline);
            let gap = gap(status, &html);
            match kind {
                MarkerKind::Blockquote => {
                    if !html.trim().is_empty() {
                        let _ = write!(self.out, "<p{item_classes}>{status}{gap}{html}</p>");
                    }
                }
                _ => {
                    let _ = write!(self.out, "<li{item_classes}>{status}{gap}{html}");
                }
            }
            self.blocks(content)?;
        }

        while let Some((open, _)) = stack.pop() {
            self.close_item(open);
            let _ = write!(self.out, "</{}>", container_tag(open));
        }
        self.out.push('\n');
        self.list_depth -= 1;
        Ok(())
    }

    fn close_item(&mut self, kind: MarkerKind) {
        if kind != MarkerKind::Blockquote {
            self.out.push_str("</li>");
        }
    }

    /// Header rows emit `<th>`, data rows `<td>`. Mog allows a header row
    /// anywhere in the table, so every row lives in one `<tbody>` rather than
    /// splitting a `<thead>` that could not hold them all.
    fn table(&mut self, node: &Node) {
        let mut html = format!("<table{}><tbody>", class_attr(node));
        for row in children(node) {
            let cell_tag = match row.kind {
                NodeKind::Delimiter(Delimiter::TableHeader) => "th",
                _ => "td",
            };
            let _ = write!(html, "<tr{}>", class_attr(row));
            for cell in children(row) {
                let content = self.inline(children(cell));
                let _ = write!(
                    html,
                    "<{cell_tag}{}>{content}</{cell_tag}>",
                    class_attr(cell)
                );
            }
            html.push_str("</tr>");
        }
        html.push_str("</tbody></table>");
        self.push_block(&html);
    }

    fn verbatim(&mut self, node: &Node) -> Result<(), EmbedParseError> {
        let args = string_args(node);
        let content = raw_text(node);

        if args.first().is_some_and(|arg| *arg == EMBED) {
            let index = self.embed_decls;
            self.embed_decls += 1;
            match embed(args.get(1).copied(), &content, self.mode, index)? {
                Some(Embedded::Css(css)) => self.css.push(css),
                Some(Embedded::Component { mode, code }) => self.push_embed(mode, code),
                None => {}
            }
            return Ok(());
        }

        let lang = args.first().copied().unwrap_or("text");
        let html = match self.highlighter.highlight_spans(lang, &content) {
            Ok(spans) => format!(
                r#"<pre class="arborium lang-{}"><code>{}</code></pre>"#,
                encode_minimal(lang),
                highlight_lines(&content, spans)
            ),
            Err(_) => format!(
                r#"<pre><code>{}</code></pre>"#,
                wrap_plain_lines(&encode_minimal(&content))
            ),
        };
        self.push_block(&html);
        Ok(())
    }

    fn push_embed(&mut self, mode: String, code: String) {
        // Splitting a part mid-list leaves `<ul><li>` in one part and `</li></ul>`
        // in the next. Every other container closes its tag before recursing, so
        // a list is the only place the split can tear a tag pair — any future
        // container that recurses with markup still open must count itself here.
        if self.list_depth > 0 {
            crate::diagnostics::warn(
                "embed inside a list item splits the surrounding list markup — \
                 move it to the top level",
            );
        }
        self.parts.push(std::mem::take(&mut self.out));
        self.embeds.push(EmbedComponent {
            index: self.embeds.len() as u32,
            mode,
            code,
        });
    }

    fn inline(&mut self, nodes: &[Node]) -> String {
        let mut out = String::new();
        let mut index = 0;

        while index < nodes.len() {
            let node = &nodes[index];
            index += 1;

            match &node.kind {
                NodeKind::Text(text) | NodeKind::Raw(text) => {
                    out.push_str(&encode_minimal(text));
                    out.push_str(&self.inline(children(node)));
                }
                // Verbatim attributes name a language, so they never become
                // classes here.
                NodeKind::Delimiter(Delimiter::Verbatim) => {
                    let _ = write!(out, "<code>{}</code>", encode_minimal(&raw_text(node)));
                }
                NodeKind::Link(target) => {
                    let name = children(node)
                        .iter()
                        .find(|child| is_delimiter(child, Delimiter::LinkName));
                    let note = children(node)
                        .iter()
                        .find(|child| is_delimiter(child, Delimiter::Footnote));

                    let display = name.map(|name| self.inline(children(name)));
                    let alt = name.map(|name| text_of(children(name)));
                    self.link(target, node, display, alt, &mut out);
                    if let Some(note) = note {
                        let reference = self.footnote(children(note));
                        out.push_str(&reference);
                    }
                    for child in children(node).iter().filter(|child| {
                        !is_delimiter(child, Delimiter::LinkName)
                            && !is_delimiter(child, Delimiter::Footnote)
                    }) {
                        out.push_str(&self.inline(std::slice::from_ref(child)));
                    }
                }
                NodeKind::Delimiter(Delimiter::Footnote) => {
                    let reference = self.footnote(children(node));
                    out.push_str(&reference);
                }
                NodeKind::Delimiter(delimiter) => {
                    let content = self.inline(children(node));
                    match inline_tag(*delimiter) {
                        Some(tag) => {
                            let _ = write!(out, "<{tag}{}>{content}</{tag}>", class_attr(node));
                        }
                        None => out.push_str(&content),
                    }
                }
                // A paragraph inside an inline run is a soft-wrapped block
                // delimiter's body — unwrap it rather than nesting a <p>.
                NodeKind::Paragraph | NodeKind::Marker(_) => {
                    let content = self.inline(children(node));
                    out.push_str(&content);
                }
                NodeKind::Table | NodeKind::Data(_) => {}
            }
        }

        out
    }

    /// `[[target]]((name)){{note}}`. The target's leading attribute carries the
    /// protocol (`[[https://kdl.dev]]`) or the transclusion marker (`[[!:x.png]]`).
    fn link(
        &mut self,
        target: &str,
        node: &Node,
        display: Option<String>,
        alt: Option<String>,
        out: &mut String,
    ) {
        let args = string_args(node);
        let transclude = args.contains(&TRANSCLUDE);
        let protocol = args.iter().find(|arg| **arg != TRANSCLUDE);

        let href = match protocol {
            Some(&"#") => format!("#{}", into_slug(target)),
            Some(protocol) => format!("{protocol}:{target}"),
            // Supports legacy targets parsed without the `#` attribute.
            None => match target.strip_prefix("#:") {
                Some(heading) => format!("#{}", into_slug(heading)),
                None => mg_to_html(target),
            },
        };

        // An unnamed link shows what it points at: the whole URL when a
        // protocol rebuilt it, otherwise the reference as written.
        let display = display.unwrap_or_else(|| match protocol {
            Some(_) => encode_minimal(&href),
            None => encode_minimal(target),
        });
        if has_unsafe_scheme(&href) {
            crate::diagnostics::warn(format!("dropping link with unsafe URL scheme: {href}"));
            out.push_str(&display);
            return;
        }

        if transclude {
            if is_image(&href) {
                let alt = alt.unwrap_or_default();
                let _ = write!(
                    out,
                    r#"<img src="{}" alt="{}" />"#,
                    encode_minimal(&relative(&href)),
                    encode_minimal(&alt)
                );
                return;
            }
            crate::diagnostics::warn(format!(
                "document transclusion is not supported — rendered as a link: {href}"
            ));
        }

        // An external page can hijack `window.opener` without the rel.
        let external = match UrlKind::of(&href).is_external() {
            true => r#" target="_blank" rel="noopener noreferrer""#,
            false => "",
        };
        let _ = write!(
            out,
            r#"<a href="{}"{external}>{display}</a>"#,
            encode_minimal(&href)
        );
    }

    /// Footnotes are numbered by reference order. A `[[name]]{{body}}` pair that
    /// defines a note for an earlier reference renders as its own note too —
    /// named resolution needs a registry mog doesn't model yet.
    fn footnote(&mut self, body: &[Node]) -> String {
        let html = self.inline(body);
        self.footnotes.push(html);
        let n = self.footnotes.len();
        format!(
            r##"<sup class="footnote-ref"><a href="#footnote-{n}" id="footnote-ref-{n}">{n}</a></sup>"##
        )
    }

    fn unique_id(&mut self, base: String) -> String {
        if base.is_empty() || self.ids.insert(base.clone()) {
            return base;
        }
        (1..)
            .map(|suffix| format!("{base}-{suffix}"))
            .find(|id| self.ids.insert(id.clone()))
            .expect("id")
    }
}

/// The separator between a task's status marker and the text beside it, absent
/// when either side is empty.
fn gap(status: &str, html: &str) -> &'static str {
    match status.is_empty() || html.is_empty() {
        true => "",
        false => " ",
    }
}

const EMBED: &str = "embed";
const TRANSCLUDE: &str = "!";

fn is_delimiter(node: &Node, delimiter: Delimiter) -> bool {
    node.kind == NodeKind::Delimiter(delimiter)
}

fn children(node: &Node) -> &[Node] {
    node.children.as_deref().unwrap_or_default()
}

fn inline_tag(delimiter: Delimiter) -> Option<&'static str> {
    match delimiter {
        Delimiter::Strong => Some("strong"),
        Delimiter::Italic => Some("em"),
        Delimiter::Strikethrough => Some("s"),
        _ => None,
    }
}

fn container_tag(kind: MarkerKind) -> &'static str {
    match kind {
        MarkerKind::OrderedList => "ol",
        MarkerKind::Blockquote => "blockquote",
        _ => "ul",
    }
}

fn list_kind(node: &Node) -> Option<(MarkerKind, usize)> {
    match &node.kind {
        NodeKind::Marker(marker) => matches!(
            marker.kind,
            MarkerKind::UnorderedList | MarkerKind::OrderedList | MarkerKind::Blockquote
        )
        .then_some((marker.kind, marker.depth)),
        _ => None,
    }
}

/// True for nodes `blocks` renders as blocks. Verbatim is always a block here —
/// `split_content` decides whether a marker's children are block content at all.
fn is_block(node: &Node) -> bool {
    matches!(
        node.kind,
        NodeKind::Paragraph
            | NodeKind::Table
            | NodeKind::Marker(_)
            | NodeKind::Data(_)
            | NodeKind::Delimiter(Delimiter::Verbatim)
    )
}

/// Splits a marker's children into its inline text and its block content. A
/// marker with text on its own line carries only inline children; a marker that
/// opened a block carries paragraphs, nested markers, tables and verbatim
/// blocks. A single-line verbatim child is inline code, not a block.
fn split_content(node: &Node) -> (&[Node], &[Node]) {
    let nodes = children(node);
    // Block form wraps the marker's own line in a paragraph, so unwrap it: the
    // whole point of `#\ntitle\n#` is to render exactly like `# title`.
    if let [first, rest @ ..] = nodes
        && matches!(first.kind, NodeKind::Paragraph)
    {
        return (children(first), rest);
    }
    let block_child = |node: &Node| match &node.kind {
        NodeKind::Delimiter(Delimiter::Verbatim) => children(node).len() > 1,
        _ => is_block(node),
    };
    match nodes.iter().position(block_child) {
        Some(split) => nodes.split_at(split),
        None => (nodes, &[]),
    }
}

/// The bare (unnamed, string-valued) attributes of a node, in source order:
/// `##red:underline:` → `["red", "underline"]`.
fn string_args(node: &Node) -> Vec<&str> {
    node.attributes
        .iter()
        .flatten()
        .filter(|data| data.name.is_none())
        .filter_map(|data: &Data| match &data.value {
            Value::String(string) => Some(string.as_str()),
            _ => None,
        })
        .collect()
}

/// Bare attributes become classes. Links and verbatim blocks never come through
/// here — theirs name a protocol or a language, and both render their own tag.
fn class_attr(node: &Node) -> String {
    classes_attr(&string_args(node).join(" "))
}

/// `encode_minimal` already covers the attribute set — `&`, `<`, `>`, `"`, `'` —
/// so quoting an attribute value needs nothing extra.
fn classes_attr(classes: &str) -> String {
    match classes.is_empty() {
        true => String::new(),
        false => format!(r#" class="{}""#, encode_minimal(classes)),
    }
}

/// A structural marker's class attribute and its task status marker. A task is
/// the marker's leading attribute (`-.: Buy milk`); it becomes a status class
/// and a rendered marker, and any further attributes stay plain classes.
fn marker_markup(node: &Node) -> (String, &'static str) {
    let args = string_args(node);
    let Some((class, status)) = args.first().copied().and_then(task_status) else {
        return (classes_attr(&args.join(" ")), "");
    };

    let classes = ["task", class]
        .into_iter()
        .chain(args[1..].iter().copied())
        .collect::<Vec<_>>()
        .join(" ");
    (classes_attr(&classes), status)
}

fn task_status(argument: &str) -> Option<(&'static str, &'static str)> {
    Some(match argument {
        "." => (
            "task-done",
            r#"<input type="checkbox" class="task-status task-done" checked disabled />"#,
        ),
        ">" => (
            "task-doing",
            r#"<span class="task-status task-doing">&#x2192;</span>"#,
        ),
        "?" => (
            "task-uncertain",
            r#"<span class="task-status task-uncertain">?</span>"#,
        ),
        "o" => (
            "task-undone",
            r#"<input type="checkbox" class="task-status task-undone" disabled />"#,
        ),
        "x" => (
            "task-cancelled",
            r#"<span class="task-status task-cancelled">&#x2715;</span>"#,
        ),
        _ => return None,
    })
}

/// A verbatim block's literal text: mog leaves its children as one `Raw` node
/// per source line.
fn raw_text(node: &Node) -> String {
    children(node)
        .iter()
        .filter_map(|child| match &child.kind {
            NodeKind::Raw(text) => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn text_of(nodes: &[Node]) -> String {
    let mut out = String::new();
    push_text(nodes, &mut out);
    out
}

fn push_text(nodes: &[Node], out: &mut String) {
    for node in nodes {
        match &node.kind {
            NodeKind::Text(text) | NodeKind::Raw(text) => out.push_str(text),
            _ => push_text(children(node), out),
        }
    }
}

/// Rewrites an in-site `.mg` reference to the `.html` it builds to. A bare
/// reference (`[[recipe]]`) names a document, so it gets the extension too.
fn mg_to_html(target: &str) -> String {
    if target.is_empty() || !UrlKind::of(target).is_site_relative() || target.starts_with('#') {
        return target.to_string();
    }
    let (path, suffix) = split_url_suffix(target);
    let rewritten = match path.strip_suffix(".mg") {
        Some(base) => format!("{base}.html"),
        None if file_extension(path).is_none() => format!("{path}.html"),
        None => path.to_string(),
    };
    rewritten + suffix
}

fn split_url_suffix(url: &str) -> (&str, &str) {
    match url
        .char_indices()
        .find(|(_, character)| matches!(character, '?' | '#'))
    {
        Some((index, _)) => url.split_at(index),
        None => (url, ""),
    }
}

fn file_extension(path: &str) -> Option<&str> {
    let name = path.rsplit('/').next()?;
    name.rsplit_once('.').map(|(_, extension)| extension)
}

/// ponytail: extension allowlist — an unlisted image format transcludes as a
/// link rather than an `<img>`. Widen the list when one shows up; nothing at
/// render time can tell without sniffing the file.
fn is_image(path: &str) -> bool {
    let (path, _) = split_url_suffix(path);
    file_extension(path).is_some_and(|extension| {
        matches!(
            extension.to_ascii_lowercase().as_str(),
            "png" | "jpg" | "jpeg" | "gif" | "svg" | "webp" | "avif" | "bmp" | "ico"
        )
    })
}

/// Only a bare relative path needs `./`; rooted, `//host` and scheme'd sources
/// already resolve.
fn relative(path: &str) -> String {
    match UrlKind::of(path).is_site_relative() && !path.starts_with('/') {
        true => format!("./{path}"),
        false => path.to_string(),
    }
}

fn highlight_lines(code: &str, mut spans: Vec<Span>) -> String {
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

fn wrap_plain_lines(text: &str) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn html(source: &str) -> String {
        render(&mog_parser::parse(source), None)
            .expect("render")
            .parts
            .join("")
    }

    #[test]
    fn nested_mixed_markers_close_in_order() {
        let out = html("- Outer\n.. Inner\n.. Inner two\n- Outer two\n");
        assert_eq!(
            out.trim(),
            "<ul><li>Outer<ol><li>Inner</li><li>Inner two</li></ol></li><li>Outer two</li></ul>"
        );
    }

    #[test]
    fn heading_ids_are_unique_and_slugged() {
        let out = html("# My Heading\n\n# My Heading\n");
        assert!(out.contains(r#"<h1 id="my-heading">"#), "{out}");
        assert!(out.contains(r#"<h1 id="my-heading-1">"#), "{out}");
    }

    #[test]
    fn link_protocol_attribute_rebuilds_the_url() {
        let out = html("[[https://kdl.dev]]((KDL))\n");
        assert_eq!(
            out.trim(),
            r#"<p><a href="https://kdl.dev" target="_blank" rel="noopener noreferrer">KDL</a></p>"#
        );
    }

    #[test]
    fn bare_link_resolves_to_a_document() {
        assert!(html("[[recipe]]\n").contains(r#"<a href="recipe.html">recipe</a>"#));
        assert!(html("[[#:Ingredients]]\n").contains(r##"<a href="#ingredients">"##));
    }

    #[test]
    fn unsafe_scheme_link_drops_to_text() {
        let out = html("[[javascript:alert(1)]]((click me))\n");
        assert!(!out.contains("<a "), "{out}");
        assert!(out.contains("click me"), "{out}");
    }

    #[test]
    fn image_transclusion_renders_an_img() {
        let out = html("[[!:relative/image.png]]((alt text))\n");
        assert!(
            out.contains(r#"<img src="./relative/image.png" alt="alt text" />"#),
            "{out}"
        );
    }

    #[test]
    fn image_alt_text_is_plain_and_attribute_encoded() {
        let out = html(r#"[[!:image.png]]((**bold** "quoted"))"#);
        assert!(out.contains(r#"alt="bold &quot;quoted&quot;""#), "{out}");
        assert!(!out.contains("<strong>"), "{out}");
    }

    #[test]
    fn unsafe_image_scheme_does_not_render_an_image() {
        let out = html("[[!:javascript:evil.png]]((blocked))\n");
        assert!(!out.contains("<img"), "{out}");
        assert!(out.contains("blocked"), "{out}");
    }

    #[test]
    fn image_transclusion_accepts_query_strings_and_fragments() {
        let out = html("[[!:image.png?width=2#preview]]((preview))\n");
        assert!(
            out.contains(r#"src="./image.png?width=2#preview""#),
            "{out}"
        );
    }

    #[test]
    fn footnotes_collect_into_one_list() {
        let out = html("tomato{{ A fruit }}\n");
        assert!(out.contains(r##"href="#footnote-1""##), "{out}");
        assert!(
            out.contains(r#"<aside class="footnotes"><ol><li id="footnote-1">A fruit"#),
            "{out}"
        );
    }

    #[test]
    fn verbatim_block_renders_pre_and_inline_renders_code() {
        let block = html("``text:\nline one\nline two\n``\n");
        assert!(block.contains("<pre"), "{block}");
        let inline = html("Some code with ``text: print()``\n");
        assert!(
            inline.contains("<p>Some code with <code>print()</code></p>"),
            "{inline}"
        );
    }

    #[test]
    fn task_attributes_render_a_status_marker() {
        let out = html("-.: done\n->: doing\n-?: uncertain\n-o: not done\n-x: cancelled\n");

        assert!(
            out.contains(
                r#"<li class="task task-done"><input type="checkbox" class="task-status task-done" checked disabled /> done</li>"#
            ),
            "{out}"
        );
        assert!(
            out.contains(r#"<li class="task task-undone"><input type="checkbox" class="task-status task-undone" disabled /> not done</li>"#),
            "{out}"
        );
        for class in ["task-doing", "task-uncertain", "task-cancelled"] {
            assert!(
                out.contains(&format!(r#"<li class="task {class}">"#)),
                "{out}"
            );
        }
    }

    #[test]
    fn a_task_keeps_its_remaining_attributes_as_classes() {
        let out = html("-.:urgent: Ship it\n");
        assert!(
            out.contains(r#"<li class="task task-done urgent">"#),
            "{out}"
        );
    }

    #[test]
    fn attributes_become_classes() {
        let out = html("##red:underline: My Heading\n");
        assert!(out.contains(r#"class="red underline""#), "{out}");
    }

    #[test]
    fn table_rows_pick_th_or_td() {
        let out = html("#| Name || Type ||\n-| Apple || Fruit ||\n");
        assert!(out.contains("<tr><th>Name</th><th>Type</th></tr>"), "{out}");
        assert!(
            out.contains("<tr><td>Apple</td><td>Fruit</td></tr>"),
            "{out}"
        );
    }

    #[test]
    fn escaped_delimiters_render_literally() {
        let out = html(r"\**this is not bold\**");
        assert!(!out.contains("<strong>"), "{out}");
        assert!(out.contains("**this is not bold**"), "{out}");
    }

    #[test]
    fn only_in_site_references_get_rewritten() {
        for target in ["mailto:me@example.mg", "ftp://host/file.mg", "#anchor"] {
            assert_eq!(mg_to_html(target), target);
        }
        assert_eq!(mg_to_html("docs/readme.mg"), "docs/readme.html");
        assert_eq!(
            mg_to_html("docs/readme.mg?raw=1#intro"),
            "docs/readme.html?raw=1#intro"
        );
        assert_eq!(mg_to_html("recipe#ingredients"), "recipe.html#ingredients");
        assert_eq!(mg_to_html("recipe"), "recipe.html");
        assert_eq!(mg_to_html("image.png"), "image.png");
    }

    #[test]
    fn toc_titles_are_plain_text() {
        let rendered =
            render(&mog_parser::parse("# **Formatted** heading\n"), None).expect("render");
        assert_eq!(rendered.toc[0].title, "Formatted heading");
    }
}
