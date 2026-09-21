//! The parser's tree, shaped for JavaScript.
//!
//! The consumers are build scripts, not browsers: they want the document, not
//! HTML. `mog_parser`'s own `Serialize` is not that shape — `NodeKind` is
//! externally tagged (`{ "Marker": {…} }` beside a bare `"Paragraph"`) and
//! `Value` is untagged, neither of which TypeScript can narrow. These mirrors
//! tag both internally on `kind`, matching how `Segment` is already exposed.
//!
//! They borrow from the parsed document rather than cloning it: the tree is
//! serialised once and never outlives the parse.

use mog_parser::{
    Attribute, Attributes, Delimiter, Document, MarkerKind, Node, NodeKind, Span, Value,
};
use serde::{Serialize, Serializer};
use serde_json::{Map, Value as Json};

/// `plain` adds the projection `metadata` uses — one argument is a scalar,
/// several an array, properties or children an object, first key wins — beside
/// the structured attributes. Opt-in: it roughly doubles the attribute payload,
/// and a consumer wants one form or the other.
pub fn document(
    document: &Document,
    plain: bool,
    diagnostics: Option<Vec<String>>,
) -> AstDocument<'_> {
    AstDocument {
        attributes: document
            .attributes
            .as_deref()
            .map(|owned| attributes(owned, plain)),
        body: AstNodes(&document.body, plain),
        diagnostics,
    }
}

/// `Span` spells its fields for Rust; the rest of this boundary is camelCase.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AstSpan {
    start: usize,
    end: usize,
    start_line: usize,
    end_line: usize,
}

impl From<&Span> for AstSpan {
    fn from(span: &Span) -> Self {
        AstSpan {
            start: span.start,
            end: span.end,
            start_line: span.start_line,
            end_line: span.end_line,
        }
    }
}

#[derive(Serialize)]
pub struct AstDocument<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    attributes: Option<AstAttributes<'a>>,
    body: AstNodes<'a>,
    #[serde(skip_serializing_if = "Option::is_none")]
    diagnostics: Option<Vec<String>>,
}

/// Serde consumes these borrowed sequences one item at a time. Only the plain
/// projection allocates; there is no second recursive tree to build or drop.
struct AstNodes<'a>(&'a [Node], bool);

impl AstNodes<'_> {
    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Serialize for AstNodes<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(self.0.iter().map(|child| node(child, self.1)))
    }
}

fn serialize_entries<S: Serializer>(
    entries: &[Attribute],
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.collect_seq(entries.iter().map(attribute))
}

fn spans<S: Serializer>(spans: &[Span], serializer: S) -> Result<S::Ok, S::Error> {
    serializer.collect_seq(spans.iter().map(AstSpan::from))
}

#[derive(Serialize)]
struct AstNode<'a> {
    #[serde(flatten)]
    kind: AstKind<'a>,
    #[serde(skip_serializing_if = "Option::is_none")]
    attributes: Option<AstAttributes<'a>>,
    #[serde(skip_serializing_if = "AstNodes::is_empty")]
    children: AstNodes<'a>,
    /// Absent on inline nodes — see `mog_parser::Node::span`.
    #[serde(skip_serializing_if = "Option::is_none")]
    span: Option<AstSpan>,
    #[serde(skip_serializing_if = "Option::is_none")]
    fence: Option<AstSpan>,
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
enum AstKind<'a> {
    /// `marker` rather than `kind`: the tag already owns that name.
    Marker {
        marker: &'static str,
        depth: usize,
    },
    Delimiter {
        delimiter: &'static str,
    },
    Raw {
        text: &'a str,
    },
    Link {
        target: &'a str,
    },
    Attributes,
    Paragraph,
    Table,
    Text {
        text: &'a str,
    },
}

#[derive(Serialize)]
struct AstAttributes<'a> {
    #[serde(
        skip_serializing_if = "<[Attribute]>::is_empty",
        serialize_with = "serialize_entries"
    )]
    entries: &'a [Attribute],
    #[serde(
        skip_serializing_if = "<[Attribute]>::is_empty",
        serialize_with = "serialize_entries"
    )]
    children: &'a [Attribute],
    /// Where the `` ``attr: `` blocks behind `children` are. Empty when none
    /// of them is spliceable — see `mog_parser::Attributes::blocks`.
    #[serde(skip_serializing_if = "<[Span]>::is_empty", serialize_with = "spans")]
    blocks: &'a [Span],
    /// `children` as plain values, by the same rule and the same code as
    /// `metadata`. Present only when asked for.
    #[serde(skip_serializing_if = "Option::is_none")]
    plain: Option<Map<String, Json>>,
}

#[derive(Serialize)]
struct AstAttribute<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ty: Option<&'a str>,
    value: AstValue<'a>,
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
enum AstValue<'a> {
    Null,
    Bool {
        value: bool,
    },
    Int {
        #[serde(serialize_with = "exact_integer")]
        value: i128,
    },
    Float {
        value: f64,
    },
    String {
        value: &'a str,
    },
    Node {
        #[serde(
            skip_serializing_if = "<[Attribute]>::is_empty",
            serialize_with = "serialize_entries"
        )]
        entries: &'a [Attribute],
        #[serde(
            skip_serializing_if = "<[Attribute]>::is_empty",
            serialize_with = "serialize_entries"
        )]
        children: &'a [Attribute],
    },
}

fn node(node: &Node, plain: bool) -> AstNode<'_> {
    AstNode {
        kind: match &node.kind {
            NodeKind::Marker(marker) => AstKind::Marker {
                marker: marker_name(marker.kind),
                depth: marker.depth,
            },
            NodeKind::Delimiter(delimiter) => AstKind::Delimiter {
                delimiter: delimiter_name(*delimiter),
            },
            NodeKind::Raw(text) => AstKind::Raw { text },
            NodeKind::Link(target) => AstKind::Link { target },
            NodeKind::Attributes => AstKind::Attributes,
            NodeKind::Paragraph => AstKind::Paragraph,
            NodeKind::Table => AstKind::Table,
            NodeKind::Text(text) => AstKind::Text { text },
        },
        attributes: node
            .attributes
            .as_deref()
            .map(|owned| attributes(owned, plain)),
        children: AstNodes(&node.children, plain),
        span: node.span.as_ref().map(AstSpan::from),
        fence: node.fence.as_ref().map(AstSpan::from),
    }
}

fn attributes(attributes: &Attributes, plain: bool) -> AstAttributes<'_> {
    AstAttributes {
        entries: &attributes.entries,
        children: &attributes.children,
        blocks: &attributes.blocks,
        // the same function `extract_metadata` calls, so the two cannot drift
        plain: (plain && !attributes.children.is_empty())
            .then(|| crate::metadata::into_map(&attributes.children)),
    }
}

fn attribute(attribute: &Attribute) -> AstAttribute<'_> {
    AstAttribute {
        name: attribute.name.as_deref(),
        ty: attribute.ty.as_deref(),
        value: value(&attribute.value),
    }
}

fn value(value: &Value) -> AstValue<'_> {
    match value {
        Value::Null => AstValue::Null,
        Value::Bool(bool) => AstValue::Bool { value: *bool },
        Value::Int(int) => AstValue::Int { value: *int },
        Value::Float(float) => AstValue::Float { value: *float },
        Value::String(string) => AstValue::String { value: string },
        Value::Node(node) => AstValue::Node {
            entries: &node.entries,
            children: &node.children,
        },
    }
}

/// Shares `metadata`'s rule: a wider KDL integer crosses as a string rather
/// than silently losing its low digits.
fn exact_integer<S: Serializer>(int: &i128, serializer: S) -> Result<S::Ok, S::Error> {
    match crate::metadata::safe_integer(*int) {
        Some(number) => serializer.serialize_i64(number),
        None => serializer.serialize_str(&int.to_string()),
    }
}

fn marker_name(kind: MarkerKind) -> &'static str {
    match kind {
        MarkerKind::Heading => "heading",
        MarkerKind::UnorderedList => "unordered-list",
        MarkerKind::OrderedList => "ordered-list",
        MarkerKind::Blockquote => "blockquote",
        MarkerKind::Free => "free",
    }
}

fn delimiter_name(delimiter: Delimiter) -> &'static str {
    match delimiter {
        Delimiter::Strong => "strong",
        Delimiter::Italic => "italic",
        Delimiter::Verbatim => "verbatim",
        Delimiter::Strikethrough => "strikethrough",
        Delimiter::TableHeader => "table-header",
        Delimiter::TableRow => "table-row",
        Delimiter::TableCell => "table-cell",
        Delimiter::Footnote => "footnote",
        Delimiter::Link => "link",
        Delimiter::LinkName => "link-name",
    }
}
