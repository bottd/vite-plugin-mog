use crate::embed::{EmbedParseError, Embedded, embed};
use crate::types::{
    ContainerTag, DataAttr, DataFilter, EmbedComponent, OutputMode, Segment, TocEntry,
};
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
            Segment::Open { tag, classes, data } => write_open(&mut out, *tag, classes, data),
            Segment::Close { tag } => write_close(&mut out, *tag),
        }
    }
    out
}

pub fn render(
    document: &Document,
    mode: Option<OutputMode>,
    data: DataFilter,
) -> Result<Rendered, EmbedParseError> {
    warn_renamed_blocks(&document.body);
    warn_inline_attachment(&document.body);
    let mut renderer = Renderer::new(mode, data);
    renderer.blocks(&document.body)?;
    Ok(renderer.finish())
}

/// What a container holds while it renders. Whether it is lifted (see
/// [`Segment`]) is only known once it closes, so every container is kept as a
/// tree and `finish` writes the embed-free ones back into the HTML around them.
enum Piece {
    Html(String),
    Embed(u32),
    Container(Container),
}

struct Container {
    tag: ContainerTag,
    classes: String,
    data: Vec<DataAttr>,
    pieces: Vec<Piece>,
    holds_embed: bool,
}

struct Renderer {
    root: Vec<Piece>,
    /// The containers around the cursor, outermost first.
    open: Vec<Container>,
    /// The HTML being written; `flush` hands it to the innermost container.
    out: String,
    embeds: Vec<EmbedComponent>,
    css: Vec<String>,
    toc: Vec<TocEntry>,
    /// Rendered footnote bodies, in reference order; emitted as one list at the
    /// end of the document.
    footnotes: Vec<String>,
    ids: HashSet<String>,
    mode: Option<OutputMode>,
    /// Which node attribute keys reach the output. See [`DataFilter`].
    data: DataFilter,
    highlighter: Highlighter,
    /// Counts every embed declaration the renderer visits (incl. CSS, `None`
    /// mode, and failing ones), giving errors their "embed #N" number.
    embed_decls: usize,
}

impl Renderer {
    fn new(mode: Option<OutputMode>, data: DataFilter) -> Self {
        Self {
            root: Vec::new(),
            open: Vec::new(),
            out: String::new(),
            embeds: Vec::new(),
            css: Vec::new(),
            toc: Vec::new(),
            footnotes: Vec::new(),
            ids: HashSet::new(),
            mode,
            data,
            highlighter: Highlighter::new(),
            embed_decls: 0,
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
        let mut segments = Vec::new();
        emit(self.root, false, &mut segments);
        // Every block ends in a newline, so whitespace is all that sits between
        // two lifted tags — and a host would wrap it in an empty element.
        segments
            .retain(|segment| !matches!(segment, Segment::Html { html } if html.trim().is_empty()));
        Rendered {
            segments,
            embeds: self.embeds,
            css: self.css.join("\n"),
            toc: self.toc,
        }
    }

    fn push_block(&mut self, html: &str) {
        self.out.push_str(html);
        self.out.push('\n');
    }

    fn pieces(&mut self) -> &mut Vec<Piece> {
        match self.open.last_mut() {
            Some(container) => &mut container.pieces,
            None => &mut self.root,
        }
    }

    fn flush(&mut self) {
        if !self.out.is_empty() {
            let html = std::mem::take(&mut self.out);
            self.pieces().push(Piece::Html(html));
        }
    }

    /// Only a component embed lifts anything, and only a framework mode mounts
    /// one. Without that, a container is just its tags, written in place.
    fn lifts(&self) -> bool {
        self.mode.is_some_and(|mode| mode != OutputMode::html)
    }

    fn open(&mut self, tag: ContainerTag, classes: String, data: Vec<DataAttr>) {
        if !self.lifts() {
            write_open(&mut self.out, tag, &classes, &data);
            return;
        }
        self.flush();
        self.open.push(Container {
            tag,
            classes,
            data,
            pieces: Vec::new(),
            holds_embed: false,
        });
    }

    fn close(&mut self, tag: ContainerTag) {
        if !self.lifts() {
            write_close(&mut self.out, tag);
            return;
        }
        self.flush();
        let container = self.open.pop().expect("open container");
        debug_assert_eq!(container.tag, tag);
        self.pieces().push(Piece::Container(container));
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
                    attrs_html(&classes, &data_attrs(node, &self.data)),
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
                self.open(
                    ContainerTag::div,
                    classes(node),
                    data_attrs(node, &self.data),
                );
                self.out.push('\n');
                self.blocks(&node.children)?;
                self.close(ContainerTag::div);
                self.out.push('\n');
            }
            NodeKind::Paragraph => self.paragraph(&node.children, &class_attr(node, &self.data)),
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
                self.close_level(open);
            }

            let (classes, status) = marker_markup(node);
            let data = data_attrs(node, &self.data);
            let (inline, content) = split_content(node);
            let html = self.inline(inline);
            let gap = gap(status, &html);
            // A marker decorates the element it produces: a `>` the blockquote
            // it always opens (the loop above closed any sibling quote), a list
            // marker its `<li>`.
            match kind {
                MarkerKind::Blockquote => {
                    self.open(ContainerTag::blockquote, classes, data);
                    stack.push((kind, depth));
                    if !html.trim().is_empty() {
                        let _ = write!(self.out, "<p>{status}{gap}{html}</p>");
                    }
                }
                _ => {
                    match stack.last() {
                        Some(&(_, level)) if level == depth => self.close(ContainerTag::li),
                        _ => {
                            self.open(container_tag(kind), String::new(), Vec::new());
                            stack.push((kind, depth));
                        }
                    }
                    self.open(ContainerTag::li, classes, data);
                    let _ = write!(self.out, "{status}{gap}{html}");
                }
            }
            self.blocks(content)?;
        }

        while let Some((open, _)) = stack.pop() {
            self.close_level(open);
        }
        self.out.push('\n');
        Ok(())
    }

    /// Closes one level of a list run: its open item, then the container.
    fn close_level(&mut self, kind: MarkerKind) {
        if kind != MarkerKind::Blockquote {
            self.close(ContainerTag::li);
        }
        self.close(container_tag(kind));
    }

    /// Header rows emit `<th>`, data rows `<td>`. Mog allows a header row
    /// anywhere in the table, so every row lives in one `<tbody>` rather than
    /// splitting a `<thead>` that could not hold them all.
    fn table(&mut self, node: &Node) {
        let mut html = format!("<table{}><tbody>", class_attr(node, &self.data));
        for row in &node.children {
            let cell_tag = match row.kind {
                NodeKind::Delimiter(Delimiter::TableHeader) => "th",
                _ => "td",
            };
            let _ = write!(html, "<tr{}>", class_attr(row, &self.data));
            for cell in &row.children {
                let content = self.inline(&cell.children);
                let _ = write!(
                    html,
                    "<{cell_tag}{}>{content}</{cell_tag}>",
                    class_attr(cell, &self.data)
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

        if is_embed(node) {
            let index = self.embed_decls;
            self.embed_decls += 1;
            match embed(args.get(1).copied(), &content, self.mode, index)? {
                Some(Embedded::Css(css)) => self.css.push(css),
                Some(Embedded::Markup(html)) => self.push_block(&html),
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

    fn push_embed(&mut self, mode: OutputMode, code: String) {
        let index = self.embeds.len() as u32;
        self.flush();
        self.pieces().push(Piece::Embed(index));
        // Marking always covers the whole stack, so a marked container's
        // ancestors are marked already.
        for container in self.open.iter_mut().rev() {
            if std::mem::replace(&mut container.holds_embed, true) {
                break;
            }
        }
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
                            let _ = write!(
                                out,
                                "<{tag}{}>{content}</{tag}>",
                                class_attr(node, &self.data)
                            );
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
                    r#"<img src="{}" alt="{}"{} />"#,
                    encode_minimal(&relative(&href)),
                    encode_minimal(&alt),
                    link_data(node, &self.data)
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
            r#"<a href="{}"{external}{}>{display}</a>"#,
            encode_minimal(&href),
            link_data(node, &self.data)
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

/// Flattens the tree into segments, lifting what has to be: a container that
/// holds an embed, and every item of a lifted list — a host wraps loose HTML in
/// an element, which a `<ul>` or `<ol>` cannot hold. Anything else is written
/// back as tags into the HTML around it.
fn emit(pieces: Vec<Piece>, lift_items: bool, segments: &mut Vec<Segment>) {
    for piece in pieces {
        match piece {
            Piece::Html(html) => html_tail(segments).push_str(&html),
            Piece::Embed(index) => segments.push(Segment::Embed { index }),
            Piece::Container(container) => {
                let Container {
                    tag,
                    classes,
                    data,
                    pieces,
                    holds_embed,
                } = container;
                if holds_embed || (lift_items && tag == ContainerTag::li) {
                    let is_list = matches!(tag, ContainerTag::ul | ContainerTag::ol);
                    segments.push(Segment::Open { tag, classes, data });
                    emit(pieces, is_list, segments);
                    segments.push(Segment::Close { tag });
                } else {
                    write_open(html_tail(segments), tag, &classes, &data);
                    emit(pieces, false, segments);
                    write_close(html_tail(segments), tag);
                }
            }
        }
    }
}

/// The HTML being written. Adjacent HTML is one segment: a host wraps each in
/// an element of its own.
fn html_tail(segments: &mut Vec<Segment>) -> &mut String {
    if !matches!(segments.last(), Some(Segment::Html { .. })) {
        segments.push(Segment::Html {
            html: String::new(),
        });
    }
    match segments.last_mut() {
        Some(Segment::Html { html }) => html,
        _ => unreachable!("just pushed"),
    }
}

fn write_open(out: &mut String, tag: ContainerTag, classes: &str, data: &[DataAttr]) {
    let _ = write!(out, "<{tag}{}>", attrs_html(classes, data));
}

fn write_close(out: &mut String, tag: ContainerTag) {
    let _ = write!(out, "</{tag}>");
}

/// `attr` replaced ``meta: and ``data:. Either still parses, as a code block
/// that yields nothing, so only a warning tells the author why. Both are also
/// plausible languages for a real sample, which renaming would delete from the
/// page — so each warns only where it used to be read: `meta` opening the
/// document, `data` at the root. Remove with the git pin (see Cargo.toml).
/// One missing blank line moves an `` ``attr: `` block between two unrelated
/// places: directly under a bullet it lands on the `<li>`, a blank line later it
/// merges into document metadata. Both are legitimate, so this is a warning
/// rather than an error — but a generator writing those blocks should hear about
/// it, and at a glance the two spellings look the same.
///
/// Only blocks written on their own lines count. An inline one
/// (`# Heading ``attr: k 1``)` is unambiguous and carries no span, which is how
/// the two are told apart.
fn warn_inline_attachment(body: &[Node]) {
    for node in body {
        let blocks = node
            .attributes
            .as_deref()
            .map(|attributes| attributes.blocks.as_slice())
            .unwrap_or_default();

        if let Some(owner) = inline_bearing(node) {
            for block in blocks {
                crate::diagnostics::warn(format!(
                    "an ``attr: block on line {} attached to the {owner} on line {}, \
                     not to the document. A blank line between them would make it \
                     document metadata instead.",
                    block.start_line + 1,
                    node.span.map_or(0, |span| span.start_line) + 1,
                ));
            }
        }

        warn_inline_attachment(&node.children);
    }
}

/// What a node is called in that warning, or `None` when a block under it is
/// unambiguous — a fence block owns what sits inside it, which is the whole
/// point of the fence.
fn inline_bearing(node: &Node) -> Option<&'static str> {
    match &node.kind {
        NodeKind::Marker(marker) => match marker.kind {
            MarkerKind::Free => None,
            MarkerKind::Heading => Some("heading"),
            MarkerKind::UnorderedList | MarkerKind::OrderedList => Some("list item"),
            MarkerKind::Blockquote => Some("blockquote"),
        },
        NodeKind::Paragraph => Some("paragraph"),
        NodeKind::Table => Some("table"),
        _ => None,
    }
}

fn warn_renamed_blocks(body: &[Node]) {
    if body.first().and_then(verbatim_lang) == Some("meta") {
        crate::diagnostics::warn(
            "``meta: is no longer front matter — rename it to ``attr:. It is \
             rendering as a code block and contributes no metadata.",
        );
    }
    if body.iter().any(|node| verbatim_lang(node) == Some("data")) {
        crate::diagnostics::warn(
            "a root-level ``data: block no longer sets attributes. If it was \
             meant to, rename it to ``attr:. If it is a code sample, name its \
             language (``kdl:, ``text:) and this warning goes away.",
        );
    }
}

fn is_embed(node: &Node) -> bool {
    verbatim_lang(node) == Some(EMBED)
}

fn verbatim_lang(node: &Node) -> Option<&str> {
    is_delimiter(node, Delimiter::Verbatim)
        .then(|| bare_args(node).next())
        .flatten()
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
    // The tree spells a one-line embed block and inline code alike. Only the
    // block can open the marker: text beside it on the marker's line comes
    // first, and in block form that text is a paragraph, unwrapped above.
    let block_child = |(index, node): (usize, &Node)| match &node.kind {
        NodeKind::Delimiter(Delimiter::Verbatim) => {
            node.children.len() > 1 || (index == 0 && is_embed(node))
        }
        _ => is_block(node),
    };
    match nodes.iter().enumerate().position(block_child) {
        Some(split) => nodes.split_at(split),
        None => (nodes, &[]),
    }
}

/// The bare (unnamed, string-valued) attributes of a node, in source order:
/// `##red:underline:` → `["red", "underline"]`.
fn string_args(node: &Node) -> Vec<&str> {
    bare_args(node).collect()
}

fn bare_args(node: &Node) -> impl Iterator<Item = &str> {
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
}

/// Bare attributes become classes. Links and verbatim blocks never come through
/// here — theirs name a protocol or a language, and both render their own tag.
fn class_attr(node: &Node, data: &DataFilter) -> String {
    attrs_html(&classes(node), &data_attrs(node, data))
}

/// A link's `` ``attr: `` block sits in its `((name))`, the one part of a link
/// that holds markup, and describes the `<a>` or `<img>` the link becomes.
fn link_data(link: &Node, data: &DataFilter) -> String {
    link.children
        .iter()
        .find(|child| is_delimiter(child, Delimiter::LinkName))
        .map(|name| data_html(&data_attrs(name, data)))
        .unwrap_or_default()
}

/// An element's class list and `data-*` attributes, as they appear in its tag.
fn attrs_html(classes: &str, data: &[DataAttr]) -> String {
    classes_attr(classes) + &data_html(data)
}

/// A node's `` ``attr: `` block as `data-*` attributes, one per top-level key:
/// a scalar as written, anything nested as JSON. The block's KDL arrives as the
/// attributes' children (see `metadata`), and merges the same way.
fn data_attrs(node: &Node, filter: &DataFilter) -> Vec<DataAttr> {
    let mut data: Vec<DataAttr> = Vec::new();
    let keys = node.attributes.iter().flat_map(|owned| &owned.children);
    for (key, entry) in keys.filter_map(|entry| Some((entry.name.as_deref()?, entry))) {
        // Filtered before validation: a warning about a key that renders
        // nowhere describes output the document no longer has.
        if !filter.allows(key) {
            continue;
        }
        // The name lands in a tag unquoted — and, lifted, in JSX, which is the
        // stricter of the two: no `.`, which HTML alone would take.
        let valid = !key.is_empty()
            && key
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'));
        // HTML attribute names ignore case, so that is how a repeat is judged.
        let name = format!("data-{}", key.to_ascii_lowercase());
        if !valid {
            crate::diagnostics::warn(format!(
                "attr key \"{key}\" is not a valid data attribute name and was dropped"
            ));
        } else if data.iter().any(|attr| attr.name == name) {
            crate::diagnostics::warn(format!(
                "attr key \"{key}\" is set more than once — the first value was kept"
            ));
        } else {
            let value = match crate::metadata::into_json(&entry.value) {
                serde_json::Value::String(string) => string,
                other => other.to_string(),
            };
            data.push(DataAttr { name, value });
        }
    }
    data
}

/// A `data-*` value, quoted with whichever quote costs less.
///
/// Anything nested renders as JSON, so a double-quoted attribute turns every
/// `"` in it into `&quot;` — six bytes for one. Single-quoting a JSON value
/// escapes nothing but `&`, which JSON does not produce. One measured attribute
/// went from 514 bytes to 284.
///
/// Both forms parse to the same `dataset` value; `<` and `>` need no escaping
/// inside a quoted attribute value, which is where the rest of the saving is.
fn data_html(data: &[DataAttr]) -> String {
    data.iter().fold(String::new(), |mut out, attr| {
        let doubles = attr.value.matches('"').count();
        let singles = attr.value.matches('\'').count();

        let _ = match doubles > singles {
            true => write!(
                out,
                " {}='{}'",
                attr.name,
                encode_single_quoted(&attr.value)
            ),
            false => write!(out, r#" {}="{}""#, attr.name, encode_minimal(&attr.value)),
        };
        out
    })
}

fn encode_single_quoted(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for char in value.chars() {
        match char {
            '&' => out.push_str("&amp;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(char),
        }
    }
    out
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
        render(&mog_parser::parse(source), mode, DataFilter::All)
            .expect("render")
            .segments
    }

    fn html(source: &str) -> String {
        segments_html(&segments(source, None))
    }

    fn html_with_warnings(source: &str) -> (String, Vec<String>) {
        crate::diagnostics::capture(|| html(source))
    }

    fn html_filtered(source: &str, data: DataFilter) -> String {
        segments_html(
            &render(&mog_parser::parse(source), None, data)
                .expect("render")
                .segments,
        )
    }

    fn warnings(source: &str) -> Vec<String> {
        crate::diagnostics::capture(|| {
            let _ = render(&mog_parser::parse(source), None, DataFilter::All);
        })
        .1
    }

    const ATTRIBUTED: &str =
        "=hero:\n``attr:\nimpact 1\nsize 2\n``\n## Abrams\n\n- one\n``attr:\nimpact 3\n``\n=\n";
    const BARE: &str = "=hero:\n## Abrams\n\n- one\n=\n";

    #[test]
    fn a_json_value_is_single_quoted_rather_than_paying_for_every_quote() {
        let html = html_filtered(
            "=card:\n``attr:\nimpact { all before=0.505 }\n``\n# A\n=\n",
            DataFilter::All,
        );

        assert!(
            html.contains(r#"data-impact='{"all":{"before":0.505}}'"#),
            "{html}"
        );
        assert!(!html.contains("&quot;"), "{html}");
    }

    #[test]
    fn a_value_with_more_apostrophes_stays_double_quoted() {
        let html = html_filtered(
            "=card:\n``attr:\nquip \"it's o'clock\"\n``\n# A\n=\n",
            DataFilter::All,
        );

        assert!(
            html.contains(r#"data-quip="it&#x27;s o&#x27;clock""#),
            "{html}"
        );
    }

    #[test]
    fn a_value_holding_both_quote_characters_escapes_whichever_it_is_quoted_with() {
        let html = html_filtered(
            "=card:\n``attr:\nmixed \"say \\\"hi\\\" o'clock\"\n``\n# A\n=\n",
            DataFilter::All,
        );

        // two double quotes against one apostrophe, so single-quoted
        assert!(
            html.contains(r#"data-mixed='say "hi" o&#39;clock'"#),
            "{html}"
        );
    }

    #[test]
    fn an_ampersand_is_escaped_in_either_quoting() {
        let html = html_filtered(
            "=card:\n``attr:\na \"x & \\\"y\\\" & z\"\n``\n# A\n=\n",
            DataFilter::All,
        );

        assert!(html.contains("&amp;"), "{html}");
    }

    #[test]
    fn filtering_every_key_renders_what_the_document_without_blocks_renders() {
        assert_eq!(
            html_filtered(ATTRIBUTED, DataFilter::None),
            html_filtered(BARE, DataFilter::None)
        );
        // and the blocks really were doing something before
        assert_ne!(
            html_filtered(ATTRIBUTED, DataFilter::All),
            html_filtered(BARE, DataFilter::All)
        );
    }

    #[test]
    fn an_allow_list_selects_by_top_level_key() {
        let html = html_filtered(
            ATTRIBUTED,
            DataFilter::Allow(["impact".to_string()].into_iter().collect()),
        );

        assert!(html.contains("data-impact"), "{html}");
        assert!(!html.contains("data-size"), "{html}");
    }

    #[test]
    fn a_filtered_key_does_not_warn_about_output_it_no_longer_has() {
        let source = "# H\n\n=x:\n``attr:\nnot.valid 1\n``\n=\n";
        let (_, warned) = crate::diagnostics::capture(|| html_filtered(source, DataFilter::All));
        let (_, quiet) = crate::diagnostics::capture(|| html_filtered(source, DataFilter::None));

        assert_eq!(warned.len(), 1, "{warned:?}");
        assert!(quiet.is_empty(), "{quiet:?}");
    }

    #[test]
    fn an_attr_block_under_a_bullet_warns_that_it_attached_to_the_item() {
        let warnings = warnings("- one\n``attr:\nimpact 1\n``\n");

        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(warnings[0].contains("list item"), "{warnings:?}");
        assert!(warnings[0].contains("line 2"), "{warnings:?}");
        assert!(warnings[0].contains("line 1"), "{warnings:?}");
    }

    #[test]
    fn a_blank_line_makes_it_metadata_without_a_warning() {
        assert!(warnings("- one\n\n``attr:\nimpact 1\n``\n").is_empty());
    }

    #[test]
    fn a_block_directly_under_a_fence_is_unambiguous_and_quiet() {
        assert!(warnings("=hero:\n``attr:\nimpact 1\n``\n## A\n=\n").is_empty());
    }

    #[test]
    fn an_inline_block_is_unambiguous_and_quiet() {
        assert!(warnings("# Heading ``attr: k 1``\n").is_empty());
    }

    /// The segments' structure with the HTML elided: `<div.card> html embed0 </div>`.
    fn shape(source: &str, mode: OutputMode) -> String {
        shape_of(&segments(source, Some(mode)))
    }

    fn shape_of(segments: &[Segment]) -> String {
        segments
            .iter()
            .map(|segment| match segment {
                Segment::Html { .. } => "html".to_string(),
                Segment::Embed { index } => format!("embed{index}"),
                Segment::Open { tag, classes, .. } if classes.is_empty() => format!("<{tag}>"),
                Segment::Open { tag, classes, .. } => format!("<{tag}.{classes}>"),
                Segment::Close { tag } => format!("</{tag}>"),
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
    fn a_nested_attribute_block_becomes_data_attributes() {
        let out = html(
            "=hero:abrams:\n``attr:\nimpact { all before=0.505 matches=21734 }\nlabel \"A & B\"\n``\n## Abrams\n=\n",
        );
        assert!(
            out.contains(
                // JSON is single-quoted (see data_html); a plain string with no
                // quote in it stays double-quoted.
                r#"<div class="hero abrams" data-impact='{"all":{"before":0.505,"matches":21734}}' data-label="A &amp; B">"#
            ),
            "{out}"
        );
    }

    #[test]
    fn a_lifted_container_keeps_its_data_attributes() {
        let rendered = render(
            &mog_parser::parse("=card:\n``attr:\nsize 3\n``\n``embed:svelte:\n<X/>\n``\n=\n"),
            Some(OutputMode::svelte),
            DataFilter::All,
        )
        .expect("render");
        assert!(
            rendered.segments.iter().any(|segment| matches!(
                segment,
                Segment::Open { data, .. }
                    if data == &[DataAttr { name: "data-size".into(), value: "3".into() }]
            )),
            "{:?}",
            rendered.segments
        );
    }

    #[test]
    fn data_attributes_keep_source_order_and_any_key_html_allows() {
        let (out, warnings) =
            html_with_warnings("=card:\n``attr:\nzeta 1\nalpha 2\n__proto__ 3\n``\ntext\n=\n");
        assert!(
            out.contains(r#"<div class="card" data-zeta="1" data-alpha="2" data-__proto__="3">"#),
            "{out}"
        );
        assert!(warnings.is_empty(), "{warnings:?}");
    }

    #[test]
    fn a_data_key_repeated_in_another_case_keeps_its_first_value() {
        let (out, warnings) = html_with_warnings("=card:\n``attr:\nFoo 1\nfoo 2\n``\ntext\n=\n");
        assert!(out.contains(r#"<div class="card" data-foo="1">"#), "{out}");
        assert_eq!(warnings.len(), 1, "{warnings:?}");
    }

    #[test]
    fn a_data_key_jsx_cannot_spell_is_dropped() {
        let (out, warnings) = html_with_warnings("=card:\n``attr:\na.b 1\n``\ntext\n=\n");
        assert!(!out.contains("data-"), "{out}");
        assert_eq!(warnings.len(), 1, "{warnings:?}");
    }

    #[test]
    fn a_link_name_carries_the_data_attributes_of_its_link() {
        let out = html(
            "[[https://kdl.dev]]((KDL ``attr: kind 1``)) [[!:pic.png]]((alt ``attr: w 3``))\n",
        );
        assert!(
            out.contains(r#"rel="noopener noreferrer" data-kind="1">KDL</a>"#),
            "{out}"
        );
        assert!(
            out.contains(r#"<img src="./pic.png" alt="alt" data-w="3" />"#),
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
            shape_of(&rendered),
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
    fn html_mode_inlines_the_embed_and_lifts_nothing() {
        let source = "=card:\nbefore\n\n``embed:html:\n<b>hi</b>\n``\n\nafter\n=\n";
        let rendered = segments(source, Some(OutputMode::html));
        assert_eq!(shape_of(&rendered), "html");
        let html = segments_html(&rendered);
        assert!(html.contains("<p>before</p>\n<b>hi</b>"), "{html}");
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
    fn inline_code_that_spells_an_embed_in_an_item_stays_code() {
        let source = "- Use ``embed:vue: <X/>`` for Vue\n";
        assert_eq!(shape(source, OutputMode::svelte), "html");
        assert!(html(source).contains("<code>&lt;X/&gt;</code> for Vue"));
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
        // Only the whitespace between blocks differs: see `finish`.
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
        let rendered = render(
            &mog_parser::parse("# **Formatted** heading\n"),
            None,
            DataFilter::All,
        )
        .expect("render");
        assert_eq!(rendered.toc[0].title, "Formatted heading");
    }
}
