use crate::embed::{CSS, EmbedParseError, Embedded, embed};
use crate::types::{ContainerTag, EmbedComponent, OutputMode, Segment, TocEntry};
use crate::utils::{UrlKind, has_unsafe_scheme, into_slug};
use arborium::advanced::{Span, spans_to_html};
use arborium::{Highlighter, HtmlFormat};
use htmlescape::encode_minimal;
use mog_parser::{Delimiter, Document, MarkerKind, Node, NodeKind, Value};
use std::collections::HashSet;
use std::fmt::Write;

pub struct Rendered {
    pub segments: Vec<Segment>,
    pub embeds: Vec<EmbedComponent>,
    pub css: String,
    pub toc: Vec<TocEntry>,
}

/// The segments as one HTML string, lifted containers written back in as tags.
/// Embeds leave no trace: what they render is the host's to supply.
#[must_use]
pub fn segments_html(segments: &[Segment]) -> String {
    let mut out = String::new();
    for segment in segments {
        match segment {
            Segment::Html { html } => out.push_str(html),
            Segment::Embed { .. } => {}
            Segment::Open { tag, classes } => out.push_str(&open_tag(*tag, classes)),
            Segment::Close { tag } => out.push_str(&close_tag(*tag)),
        }
    }
    out
}

pub fn render(document: &Document, mode: Option<OutputMode>) -> Result<Rendered, EmbedParseError> {
    let mut renderer = Renderer::new(mode);
    // Without a mode no embed mounts, so nothing needs lifting.
    if mode.is_some() {
        mark_embed_ancestors(&document.body, &mut renderer.lifted);
    }
    if document
        .body
        .first()
        .is_some_and(|node| verbatim_lang(node) == Some("meta"))
    {
        crate::diagnostics::warn(
            "``meta: is no longer front matter — rename it to ``attr:. It is \
             rendering as a code block and contributes no metadata.",
        );
    }
    renderer.blocks(&document.body)?;
    Ok(renderer.finish())
}

struct Renderer {
    segments: Vec<Segment>,
    /// The `Html` segment being written; `flush` closes it.
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
    /// How many lists and free blocks enclose the cursor.
    container_depth: usize,
    /// The containers with a component embed somewhere inside — see [`Segment`].
    /// Keyed by address: the document outlives the renderer and is never moved.
    lifted: HashSet<*const Node>,
}

impl Renderer {
    fn new(mode: Option<OutputMode>) -> Self {
        Self {
            segments: Vec::new(),
            out: String::new(),
            embeds: Vec::new(),
            css: Vec::new(),
            toc: Vec::new(),
            footnotes: Vec::new(),
            ids: HashSet::new(),
            mode,
            highlighter: Highlighter::new(),
            embed_decls: 0,
            container_depth: 0,
            lifted: HashSet::new(),
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
        self.flush();
        Rendered {
            segments: self.segments,
            embeds: self.embeds,
            css: self.css.join("\n"),
            toc: self.toc,
        }
    }

    fn push_block(&mut self, html: &str) {
        self.out.push_str(html);
        self.out.push('\n');
    }

    fn flush(&mut self) {
        let html = std::mem::take(&mut self.out);
        // Every block ends in a newline, so whitespace is all that sits between
        // two lifted tags — and a host would wrap it in an empty element.
        if !html.trim().is_empty() {
            self.segments.push(Segment::Html { html });
        }
    }

    fn lifts(&self, node: &Node) -> bool {
        self.lifted.contains(&std::ptr::from_ref(node))
    }

    fn open(&mut self, lifted: bool, tag: ContainerTag, classes: &str) {
        if lifted {
            self.flush();
            self.segments.push(Segment::Open {
                tag,
                classes: classes.to_string(),
            });
        } else {
            self.out.push_str(&open_tag(tag, classes));
        }
    }

    fn close(&mut self, lifted: bool, tag: ContainerTag) {
        if lifted {
            self.flush();
            self.segments.push(Segment::Close { tag });
        } else {
            self.out.push_str(&close_tag(tag));
        }
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
                    "<h{level}{id_attr}{}>{status}{}{title_html}</h{level}>",
                    classes_attr(&classes),
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
                let lifted = self.lifts(node);
                self.open(lifted, ContainerTag::div, &classes(node));
                self.out.push('\n');
                self.container_depth += 1;
                self.blocks(&node.children)?;
                self.container_depth -= 1;
                self.close(lifted, ContainerTag::div);
                self.out.push('\n');
            }
            NodeKind::Paragraph => self.paragraph(&node.children, &class_attr(node)),
            NodeKind::Table => self.table(node),
            NodeKind::Delimiter(Delimiter::Verbatim) => self.verbatim(node)?,
            // Nothing else reaches here: `blocks` takes list markers as runs
            // and inline nodes as paragraph runs before dispatching, and an
            // ``attr: block is consumed into its owner's attributes rather
            // than left in the body.
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
        // One decision for the whole run: a lifted `<ul>` can hold nothing but
        // real `<li>` elements, so an embed in one item lifts its siblings too.
        let lifted = items.iter().any(|item| self.lifts(item));
        self.container_depth += 1;

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
                self.close_item(lifted, open);
                self.close(lifted, container_tag(open));
            }

            let (classes, status) = marker_markup(node);
            // A marker decorates the element it produces: a list marker its
            // `<li>`, a `>` the blockquote it always opens.
            let (container_classes, item_classes) = match kind {
                MarkerKind::Blockquote => (classes.as_str(), ""),
                _ => ("", classes.as_str()),
            };
            match stack.last() {
                Some(&(_, level)) if level == depth => self.close_item(lifted, kind),
                _ => {
                    self.open(lifted, container_tag(kind), container_classes);
                    stack.push((kind, depth));
                }
            }

            let (inline, content) = split_content(node);
            let html = self.inline(inline);
            let gap = gap(status, &html);
            match kind {
                MarkerKind::Blockquote => {
                    if !html.trim().is_empty() {
                        let _ = write!(
                            self.out,
                            "<p{}>{status}{gap}{html}</p>",
                            classes_attr(item_classes)
                        );
                    }
                }
                _ => {
                    self.open(lifted, ContainerTag::li, item_classes);
                    let _ = write!(self.out, "{status}{gap}{html}");
                }
            }
            self.blocks(content)?;
        }

        while let Some((open, _)) = stack.pop() {
            self.close_item(lifted, open);
            self.close(lifted, container_tag(open));
        }
        self.out.push('\n');
        self.container_depth -= 1;
        Ok(())
    }

    fn close_item(&mut self, lifted: bool, kind: MarkerKind) {
        if kind != MarkerKind::Blockquote {
            self.close(lifted, ContainerTag::li);
        }
    }

    /// Header rows emit `<th>`, data rows `<td>`. Mog allows a header row
    /// anywhere in the table, so every row lives in one `<tbody>` rather than
    /// splitting a `<thead>` that could not hold them all.
    fn table(&mut self, node: &Node) {
        let mut html = format!("<table{}><tbody>", class_attr(node));
        for row in &node.children {
            let cell_tag = match row.kind {
                NodeKind::Delimiter(Delimiter::TableHeader) => "th",
                _ => "td",
            };
            let _ = write!(html, "<tr{}>", class_attr(row));
            for cell in &row.children {
                let content = self.inline(&cell.children);
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
        self.warn_if_renamed(lang);
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

    /// `attr` replaced ``meta: and ``data: upstream. A document written for the
    /// older parser still parses, so nothing fails — the block just renders as
    /// code and yields nothing. Only naming it tells the author why.
    ///
    /// Both are also plausible languages for a real code sample, which renaming
    /// would silently delete from the page, so each warns only where the old
    /// parser read it: `meta` opening the document (see `render`), `data` at
    /// the root. Remove with the `mog-parser` git pin (see Cargo.toml).
    fn warn_if_renamed(&self, lang: &str) {
        if lang == "data" && self.container_depth == 0 {
            crate::diagnostics::warn(
                "a root-level ``data: block no longer sets attributes. If it was \
                 meant to, rename it to ``attr:; a code sample needs no change.",
            );
        }
    }

    fn push_embed(&mut self, mode: OutputMode, code: String) {
        let index = self.embeds.len() as u32;
        self.flush();
        self.segments.push(Segment::Embed { index });
        self.embeds.push(EmbedComponent { index, mode, code });
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
                    out.push_str(&self.inline(&node.children));
                }
                // Verbatim attributes name a language, so they never become
                // classes here.
                NodeKind::Delimiter(Delimiter::Verbatim) => {
                    let _ = write!(out, "<code>{}</code>", encode_minimal(&raw_text(node)));
                }
                NodeKind::Link(target) => {
                    let name = node
                        .children
                        .iter()
                        .find(|child| is_delimiter(child, Delimiter::LinkName));
                    let note = node
                        .children
                        .iter()
                        .find(|child| is_delimiter(child, Delimiter::Footnote));

                    let display = name.map(|name| self.inline(&name.children));
                    let alt = name.map(|name| text_of(&name.children));
                    self.link(target, node, display, alt, &mut out);
                    if let Some(note) = note {
                        let reference = self.footnote(&note.children);
                        out.push_str(&reference);
                    }
                    for child in node.children.iter().filter(|child| {
                        !is_delimiter(child, Delimiter::LinkName)
                            && !is_delimiter(child, Delimiter::Footnote)
                    }) {
                        out.push_str(&self.inline(std::slice::from_ref(child)));
                    }
                }
                NodeKind::Delimiter(Delimiter::Footnote) => {
                    let reference = self.footnote(&node.children);
                    out.push_str(&reference);
                }
                NodeKind::Delimiter(delimiter) => {
                    let content = self.inline(&node.children);
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
                    let content = self.inline(&node.children);
                    out.push_str(&content);
                }
                // `parse` folds every attribute node into its owner; one survives
                // only in `parse_unfolded` trees, which the plugin never renders.
                NodeKind::Table | NodeKind::Attributes => {}
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

fn inline_tag(delimiter: Delimiter) -> Option<&'static str> {
    match delimiter {
        Delimiter::Strong => Some("strong"),
        Delimiter::Italic => Some("em"),
        Delimiter::Strikethrough => Some("s"),
        _ => None,
    }
}

fn container_tag(kind: MarkerKind) -> ContainerTag {
    match kind {
        MarkerKind::OrderedList => ContainerTag::ol,
        MarkerKind::Blockquote => ContainerTag::blockquote,
        _ => ContainerTag::ul,
    }
}

/// Records every container between the root and a component embed, returning
/// whether `nodes` hold one. It walks the tree exactly as `blocks` does, so only
/// a verbatim that will really reach `Renderer::verbatim` counts — inline code
/// spelling `embed:` does not. CSS embeds only collect a stylesheet, so they
/// leave the markup whole.
fn mark_embed_ancestors(nodes: &[Node], lifted: &mut HashSet<*const Node>) -> bool {
    let mut any = false;
    for node in nodes {
        let holds = match &node.kind {
            NodeKind::Delimiter(Delimiter::Verbatim) => {
                matches!(string_args(node)[..], [EMBED, lang, ..] if lang != CSS)
            }
            NodeKind::Marker(marker) if marker.kind == MarkerKind::Free => {
                mark_embed_ancestors(&node.children, lifted)
            }
            NodeKind::Marker(_) => mark_embed_ancestors(split_content(node).1, lifted),
            _ => false,
        };
        // A heading is no container, but marking it is harmless: nothing asks.
        if holds {
            lifted.insert(std::ptr::from_ref(node));
        }
        any |= holds;
    }
    any
}

fn verbatim_lang(node: &Node) -> Option<&str> {
    is_delimiter(node, Delimiter::Verbatim)
        .then(|| string_args(node).first().copied())
        .flatten()
}

fn open_tag(tag: ContainerTag, classes: &str) -> String {
    format!("<{}{}>", tag.as_str(), classes_attr(classes))
}

fn close_tag(tag: ContainerTag) -> String {
    format!("</{}>", tag.as_str())
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
            | NodeKind::Delimiter(Delimiter::Verbatim)
    )
}

/// Splits a marker's children into its inline text and its block content. A
/// marker with text on its own line carries only inline children; a marker that
/// opened a block carries paragraphs, nested markers, tables and verbatim
/// blocks. A single-line verbatim child is inline code, not a block — unless it
/// is an embed, which has no inline form and would otherwise render as code.
fn split_content(node: &Node) -> (&[Node], &[Node]) {
    let nodes = node.children.as_slice();
    // Block form wraps the marker's own line in a paragraph, so unwrap it: the
    // whole point of `#\ntitle\n#` is to render exactly like `# title`.
    if let [first, rest @ ..] = nodes
        && matches!(first.kind, NodeKind::Paragraph)
    {
        return (&first.children, rest);
    }
    let block_child = |node: &Node| match &node.kind {
        NodeKind::Delimiter(Delimiter::Verbatim) => {
            node.children.len() > 1 || verbatim_lang(node) == Some(EMBED)
        }
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
        // A chain's entries, not an ``attr: block's children: the two are
        // separate namespaces, and only a chain spells a bare argument.
        .flat_map(|attributes| &attributes.entries)
        .filter(|entry| entry.name.is_none())
        .filter_map(|entry| match &entry.value {
            Value::String(string) => Some(string.as_str()),
            _ => None,
        })
        .collect()
}

/// Bare attributes become classes. Links and verbatim blocks never come through
/// here — theirs name a protocol or a language, and both render their own tag.
fn class_attr(node: &Node) -> String {
    classes_attr(&classes(node))
}

fn classes(node: &Node) -> String {
    string_args(node).join(" ")
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
        return (args.join(" "), "");
    };

    let classes = ["task", class]
        .into_iter()
        .chain(args[1..].iter().copied())
        .collect::<Vec<_>>()
        .join(" ");
    (classes, status)
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
    node.children
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
            _ => push_text(&node.children, out),
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

    fn segments(source: &str, mode: Option<OutputMode>) -> Vec<Segment> {
        render(&mog_parser::parse(source), mode)
            .expect("render")
            .segments
    }

    fn html(source: &str) -> String {
        segments_html(&segments(source, None))
    }

    fn html_with_warnings(source: &str) -> (String, Vec<String>) {
        crate::diagnostics::capture(|| html(source))
    }

    /// The segments' structure with the HTML elided: `<div.card> html embed0 </div>`.
    fn shape(source: &str, mode: OutputMode) -> String {
        segments(source, Some(mode))
            .iter()
            .map(|segment| match segment {
                Segment::Html { .. } => "html".to_string(),
                Segment::Embed { index } => format!("embed{index}"),
                Segment::Open { tag, classes } if classes.is_empty() => {
                    format!("<{}>", tag.as_str())
                }
                Segment::Open { tag, classes } => format!("<{}.{classes}>", tag.as_str()),
                Segment::Close { tag } => format!("</{}>", tag.as_str()),
            })
            .collect::<Vec<_>>()
            .join(" ")
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
    fn a_renamed_attribute_block_warns_instead_of_going_quiet() {
        for lang in ["meta", "data"] {
            let source = format!("``{lang}:\ntitle \"x\"\n``\n");
            let (rendered, warnings) = html_with_warnings(&source);

            assert!(rendered.contains("<pre"));
            assert!(
                warnings.iter().any(|warning| warning.contains("``attr:")),
                "{lang}: {warnings:?}"
            );
        }
    }

    #[test]
    fn an_attribute_block_is_consumed_rather_than_rendered() {
        let (rendered, warnings) = html_with_warnings("``attr:\ntitle \"x\"\n``\n");
        assert_eq!(rendered, "");
        assert!(warnings.is_empty(), "{warnings:?}");
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

    const EMBED: &str = "``embed:svelte:\n<b>hi</b>\n``\n";

    #[test]
    fn an_embed_sits_inside_the_free_block_that_holds_it() {
        let source = format!("=card:\nbefore\n\n{EMBED}\nafter\n=\n");
        assert_eq!(
            shape(&source, OutputMode::svelte),
            "<div.card> html embed0 html </div>"
        );
    }

    #[test]
    fn every_ancestor_of_an_embed_is_lifted_and_no_other_container() {
        let source = format!("=aside:\ntext\n=\n\n=outer:\n=inner:\n{EMBED}=\n=\n");
        assert_eq!(
            shape(&source, OutputMode::svelte),
            "html <div.outer> <div.inner> embed0 </div> </div>"
        );
    }

    #[test]
    fn no_segment_is_blank() {
        let source = format!("{EMBED}\n=card:\n{EMBED}=\n\n{EMBED}");
        let rendered = segments(&source, Some(OutputMode::svelte));
        assert_eq!(
            shape(&source, OutputMode::svelte),
            "embed0 <div.card> embed1 </div> embed2"
        );
        assert!(
            !rendered
                .iter()
                .any(|segment| matches!(segment, Segment::Html { html } if html.trim().is_empty()))
        );
    }

    #[test]
    fn an_embed_in_a_list_lifts_the_whole_list() {
        let source = format!("=card:\n. one\n.\ntwo\n{EMBED}.\n. three\n=\n");
        assert_eq!(
            shape(&source, OutputMode::svelte),
            "<div.card> <ol> <li> html </li> <li> html embed0 </li> <li> html </li> </ol> </div>"
        );
    }

    #[test]
    fn a_one_line_embed_opening_an_item_still_mounts() {
        let source = "-\n``embed:svelte:\n<Counter />\n``\n-\n";
        assert_eq!(
            shape(source, OutputMode::svelte),
            "<ul> <li> embed0 </li> </ul>"
        );
    }

    #[test]
    fn inline_code_that_spells_an_embed_lifts_nothing() {
        let source = "=card:\nUse ``embed:svelte: x`` here\n=\n";
        assert_eq!(shape(source, OutputMode::svelte), "html");
    }

    #[test]
    fn a_task_item_keeps_its_classes_when_lifted() {
        let source = format!("-.:\ndone\n{EMBED}-\n");
        assert_eq!(
            shape(&source, OutputMode::svelte),
            "<ul> <li.task task-done> html embed0 </li> </ul>"
        );
    }

    #[test]
    fn lifted_markup_matches_the_unlifted_rendering() {
        let source = format!("=card:\n- one\n-\ntwo\n{EMBED}-\n=\n\n> quote\n");
        let lifted = segments_html(&segments(&source, Some(OutputMode::svelte)));
        // Only the whitespace between blocks differs: see `flush`.
        assert_eq!(lifted.replace('\n', ""), html(&source).replace('\n', ""));
        assert!(lifted.contains(r#"<div class="card">"#), "{lifted}");
    }

    #[test]
    fn stale_front_matter_warns_where_the_old_parser_read_it() {
        let (_, warnings) = html_with_warnings("``meta:\ntitle \"Old\"\n``\n\n# Heading\n");
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(warnings[0].contains("``attr:"), "{warnings:?}");

        let (_, warnings) = html_with_warnings("# Heading\n\n``data:\nkey 1\n``\n");
        assert_eq!(warnings.len(), 1, "{warnings:?}");

        // A CSS embed renders nothing, so what follows it is no longer "first"
        // by output — only by position, which is what the old parser went by.
        let (_, warnings) =
            html_with_warnings("``embed:css:\n.a {}\n``\n\n``meta:\ntitle \"Sample\"\n``\n");
        assert!(warnings.is_empty(), "{warnings:?}");
    }

    #[test]
    fn a_meta_or_data_code_sample_does_not_warn() {
        let (_, warnings) = html_with_warnings(
            "# Heading\n\n``meta:\ntitle \"Sample\"\n``\n\n=card:\n``data:\nkey 1\n``\n=\n",
        );
        assert!(warnings.is_empty(), "{warnings:?}");
    }

    #[test]
    fn toc_titles_are_plain_text() {
        let rendered =
            render(&mog_parser::parse("# **Formatted** heading\n"), None).expect("render");
        assert_eq!(rendered.toc[0].title, "Formatted heading");
    }
}
