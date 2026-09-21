use mog_parser::{Attributes, Delimiter, MarkerKind, Node, NodeKind};

use super::{is_delimiter, verbatim_lang};
use crate::diagnostics::warn;

pub(crate) fn check(nodes: &[Node]) {
    warn_renamed_blocks(nodes);
    warn_attribute_blocks(nodes, 1);
}

// An inline node has no span: use the nearest source-positioned ancestor.
fn warn_attribute_blocks(nodes: &[Node], line: usize) {
    for node in nodes {
        let line = node.span.map_or(line, |span| span.start_line + 1);
        warn_unparsed_attr_block(node, line);
        if let Some(owner) = inline_bearing(node) {
            for block in node.attributes.iter().flat_map(|owned| &owned.blocks) {
                warn(format!(
                    "an ``attr: block on line {} attached to the {owner} on line {line}, \
                     not to the document. A blank line between them would make it \
                     document metadata instead.",
                    block.start_line + 1,
                ));
            }
        }
        warn_attribute_blocks(&node.children, line);
    }
}

fn warn_unparsed_attr_block(node: &Node, line: usize) {
    if !is_delimiter(node, Delimiter::Verbatim)
        || !node
            .attributes
            .as_deref()
            .is_some_and(Attributes::is_attribute_chain)
    {
        return;
    }
    let reason = match mog_parser::parse_attribute_block(&node.raw_text()) {
        Ok(_) => "it was not read as attributes".to_string(),
        Err(error) => error.replace('\n', " "),
    };
    let (what, rendered) = match node.span {
        Some(_) => ("the ``attr: block", "a code block"),
        None => ("the inline ``attr: block", "inline code"),
    };
    warn(format!(
        "{what} on line {line} is not valid KDL, so it rendered as {rendered} \
         and set no attributes: {reason}"
    ));
}

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

// Migration warnings remain scoped to positions where meta/data were consumed.
fn warn_renamed_blocks(body: &[Node]) {
    if body.first().and_then(verbatim_lang) == Some("meta") {
        warn(
            "``meta: is no longer front matter — rename it to ``attr:. It is \
             rendering as a code block and contributes no metadata.",
        );
    }
    if body.iter().any(|node| verbatim_lang(node) == Some("data")) {
        warn(
            "a root-level ``data: block no longer sets attributes. If it was \
             meant to, rename it to ``attr:. If it is a code sample, name its \
             language (``kdl:, ``text:) and this warning goes away.",
        );
    }
}
