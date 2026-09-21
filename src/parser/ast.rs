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

/// JavaScript's exact-integer ceiling. A wider KDL integer crosses as a string
/// rather than silently losing its low digits — the same rule `metadata` uses.
const SAFE_INTEGER: i128 = 9_007_199_254_740_991;

pub fn document(document: &Document) -> AstDocument<'_> {
    AstDocument {
        attributes: document.attributes.as_deref().map(attributes),
        body: document.body.iter().map(node).collect(),
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
    body: Vec<AstNode<'a>>,
}

#[derive(Serialize)]
struct AstNode<'a> {
    #[serde(flatten)]
    kind: AstKind<'a>,
    #[serde(skip_serializing_if = "Option::is_none")]
    attributes: Option<AstAttributes<'a>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    children: Vec<AstNode<'a>>,
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
    #[serde(skip_serializing_if = "Vec::is_empty")]
    entries: Vec<AstAttribute<'a>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    children: Vec<AstAttribute<'a>>,
    /// Where the `` ``attr: `` blocks behind `children` are. Empty when none
    /// of them is spliceable — see `mog_parser::Attributes::blocks`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    blocks: Vec<AstSpan>,
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
        #[serde(skip_serializing_if = "Vec::is_empty")]
        entries: Vec<AstAttribute<'a>>,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        children: Vec<AstAttribute<'a>>,
    },
}

fn node(node: &Node) -> AstNode<'_> {
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
        attributes: node.attributes.as_deref().map(attributes),
        children: node.children.iter().map(self::node).collect(),
        span: node.span.as_ref().map(AstSpan::from),
        fence: node.fence.as_ref().map(AstSpan::from),
    }
}

fn attributes(attributes: &Attributes) -> AstAttributes<'_> {
    AstAttributes {
        entries: attributes.entries.iter().map(attribute).collect(),
        children: attributes.children.iter().map(attribute).collect(),
        blocks: attributes.blocks.iter().map(AstSpan::from).collect(),
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
            entries: node.entries.iter().map(attribute).collect(),
            children: node.children.iter().map(attribute).collect(),
        },
    }
}

fn exact_integer<S: Serializer>(int: &i128, serializer: S) -> Result<S::Ok, S::Error> {
    match int.unsigned_abs() <= SAFE_INTEGER as u128 {
        true => serializer.serialize_i64(*int as i64),
        false => serializer.serialize_str(&int.to_string()),
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
