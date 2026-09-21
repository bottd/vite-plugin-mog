//! Shared document interpretation, independent of HTML or framework output.
use mog_parser::{Delimiter, MarkerKind, Node, NodeKind, Value};
use std::collections::HashSet;

use crate::{EmbedParseError, TocEntry, embed::embed, utils::into_slug};

mod warnings;
pub(crate) use warnings::check;

#[derive(Default)]
pub(crate) struct Headings(HashSet<String>);

impl Headings {
    pub(crate) fn entry(&mut self, depth: usize, title: String) -> TocEntry {
        let base = into_slug(&title);
        let id = if base.is_empty() || self.0.insert(base.clone()) {
            base
        } else {
            (1..)
                .map(|suffix| format!("{base}-{suffix}"))
                .find(|id| self.0.insert(id.clone()))
                .expect("unique heading id")
        };
        TocEntry {
            level: depth.min(6) as u32,
            title,
            id,
        }
    }
}

/// Collects the outline and validates embeds without rendering their bodies.
pub(crate) fn outline(nodes: &[Node]) -> Result<Vec<TocEntry>, EmbedParseError> {
    fn visit(
        nodes: &[Node],
        headings: &mut Headings,
        toc: &mut Vec<TocEntry>,
        embeds: &mut usize,
    ) -> Result<(), EmbedParseError> {
        for node in nodes {
            match &node.kind {
                NodeKind::Marker(marker) => {
                    let (title, content) = split_content(node);
                    if marker.kind == MarkerKind::Heading {
                        let entry = headings.entry(marker.depth, text_of(title));
                        if !entry.id.is_empty() {
                            toc.push(entry);
                        }
                    }
                    let content = if marker.kind == MarkerKind::Free {
                        &node.children
                    } else {
                        content
                    };
                    visit(content, headings, toc, embeds)?;
                }
                NodeKind::Delimiter(Delimiter::Verbatim) if is_embed(node) => {
                    embed(bare_args(node).nth(1), "", None, *embeds)?;
                    *embeds += 1;
                }
                _ => {}
            }
        }
        Ok(())
    }

    let mut toc = Vec::new();
    visit(nodes, &mut Headings::default(), &mut toc, &mut 0)?;
    Ok(toc)
}

pub(crate) fn is_delimiter(node: &Node, delimiter: Delimiter) -> bool {
    node.kind == NodeKind::Delimiter(delimiter)
}

pub(crate) fn verbatim_lang(node: &Node) -> Option<&str> {
    is_delimiter(node, Delimiter::Verbatim)
        .then(|| bare_args(node).next())
        .flatten()
}

pub(crate) fn is_embed(node: &Node) -> bool {
    verbatim_lang(node) == Some("embed")
}

pub(crate) fn is_block(node: &Node) -> bool {
    matches!(
        node.kind,
        NodeKind::Paragraph
            | NodeKind::Table
            | NodeKind::Marker(_)
            | NodeKind::Delimiter(Delimiter::Verbatim)
    )
}

/// A marker's inline title and block body. Block titles arrive as a paragraph;
/// verbatim spans distinguish fenced blocks from inline code without guessing
/// from their content length or language.
pub(crate) fn split_content(node: &Node) -> (&[Node], &[Node]) {
    let nodes = node.children.as_slice();
    if let [first, rest @ ..] = nodes
        && matches!(first.kind, NodeKind::Paragraph)
    {
        return (&first.children, rest);
    }
    let split = nodes
        .iter()
        .position(|node| match node.kind {
            NodeKind::Delimiter(Delimiter::Verbatim) => node.span.is_some(),
            _ => is_block(node),
        })
        .unwrap_or(nodes.len());
    nodes.split_at(split)
}

pub(crate) fn bare_args(node: &Node) -> impl Iterator<Item = &str> {
    node.attributes
        .iter()
        .flat_map(|attributes| &attributes.entries)
        .filter(|entry| entry.name.is_none())
        .filter_map(|entry| match &entry.value {
            Value::String(string) => Some(string.as_str()),
            _ => None,
        })
}

pub(crate) fn text_of(nodes: &[Node]) -> String {
    fn push(nodes: &[Node], out: &mut String) {
        for node in nodes {
            match &node.kind {
                NodeKind::Text(text) | NodeKind::Raw(text) => out.push_str(text),
                _ => push(&node.children, out),
            }
        }
    }
    let mut out = String::new();
    push(nodes, &mut out);
    out
}
