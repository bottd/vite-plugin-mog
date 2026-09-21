use mog_parser::{Attributes, Delimiter, MarkerKind, Node, NodeKind};

use super::{is_delimiter, verbatim_lang};
use crate::diagnostics::warn;

pub(crate) fn check(nodes: &[Node]) {
    warn_renamed_blocks(nodes);
    warn_attribute_blocks(nodes, 1, None);
}

// An inline node has no span: use the nearest source-positioned ancestor.
fn warn_attribute_blocks(nodes: &[Node], line: usize, enclosing_block: Option<usize>) {
    for node in nodes {
        let line = node.span.map_or(line, |span| span.start_line + 1);
        warn_unparsed_attr_block(node, line);
        if let Some(owner) = inline_bearing(node) {
            // Folded trees record the blocks on the owner; unfolded trees keep
            // them as children. Both forms describe the same attachments.
            let blocks = node
                .attributes
                .iter()
                .flat_map(|owned| &owned.blocks)
                .chain(node.children.iter().filter_map(|child| match child.kind {
                    NodeKind::Attributes => child.span.as_ref(),
                    _ => None,
                }));
            for block in blocks {
                let advice = match enclosing_block {
                    Some(opening) => format!(
                        "not to the block opened on line {opening}. A blank line between them \
                         would attach it to that block instead."
                    ),
                    None => "not to the document. A blank line between them would make it \
                             document metadata instead."
                        .to_string(),
                };
                warn(format!(
                    "an ``attr: block on line {} attached to the {owner} on line {line}, {advice}",
                    block.start_line + 1,
                ));
            }
        }
        let enclosing_block = match &node.kind {
            NodeKind::Marker(marker) if marker.kind == MarkerKind::Free => Some(line),
            _ => enclosing_block,
        };
        warn_attribute_blocks(&node.children, line, enclosing_block);
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
    if body
        .iter()
        .find(|node| !matches!(node.kind, NodeKind::Attributes))
        .and_then(verbatim_lang)
        == Some("meta")
    {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics;

    #[test]
    fn attachment_advice_names_the_target_for_every_owner_and_tree_form() {
        for (opening, owner) in [
            ("# Heading", "heading"),
            ("- item", "list item"),
            (". item", "list item"),
            ("> quote", "blockquote"),
            ("paragraph", "paragraph"),
            ("-| cell", "table"),
        ] {
            for (prefix, suffix, owner_line, enclosing) in [
                ("", "", 1, None),
                ("=hero:abrams:\n", "=\n", 2, Some(1)),
                ("=\n", "=\n", 2, Some(1)),
                ("=hero:abrams:\n=ability:\n", "=\n=\n", 3, Some(2)),
            ] {
                let source = format!("{prefix}{opening}\n``attr:\nk 1\n``\n{suffix}");
                let advice = match enclosing {
                    Some(line) => format!(
                        "not to the block opened on line {line}. A blank line between them \
                         would attach it to that block instead."
                    ),
                    None => "not to the document. A blank line between them would make it \
                             document metadata instead."
                        .to_string(),
                };
                let expected = format!(
                    "an ``attr: block on line {} attached to the {owner} on line {owner_line}, {advice}",
                    owner_line + 1,
                );
                for tree in [
                    mog_parser::parse(&source),
                    mog_parser::parse_unfolded(&source),
                ] {
                    let (_, warnings) = diagnostics::capture(|| check(&tree.body));
                    assert_eq!(
                        warnings.as_slice(),
                        std::slice::from_ref(&expected),
                        "for {source:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn an_enclosing_block_does_not_leak_into_following_root_siblings() {
        let source = "=hero:\n# Inner\n``attr:\nk 1\n``\n=\n# Root\n``attr:\nk 2\n``\n";
        let tree = mog_parser::parse(source);
        let (_, warnings) = diagnostics::capture(|| check(&tree.body));
        assert_eq!(warnings.len(), 2);
        assert!(warnings[0].contains("block opened on line 1"));
        assert!(warnings[1].contains("document metadata instead."));
    }
}
