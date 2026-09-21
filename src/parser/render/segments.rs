use crate::types::{ContainerTag, DataAttr, Segment};
use std::fmt::Write;

use super::attrs_html;

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

/// Buffers containers until their embed ancestry is known. Plain HTML modes
/// write directly; framework modes lift only the ancestors an embed needs.
pub(super) struct Segments {
    root: Vec<Piece>,
    open: Vec<Container>,
    lift: bool,
}

impl Segments {
    pub(super) fn new(lift: bool) -> Self {
        Self {
            root: Vec::new(),
            open: Vec::new(),
            lift,
        }
    }

    fn pieces(&mut self) -> &mut Vec<Piece> {
        match self.open.last_mut() {
            Some(container) => &mut container.pieces,
            None => &mut self.root,
        }
    }

    fn flush(&mut self, out: &mut String) {
        if !out.is_empty() {
            self.pieces().push(Piece::Html(std::mem::take(out)));
        }
    }

    pub(super) fn open(
        &mut self,
        out: &mut String,
        tag: ContainerTag,
        classes: String,
        data: Vec<DataAttr>,
    ) {
        if !self.lift {
            write_open(out, tag, &classes, &data);
            return;
        }
        self.flush(out);
        self.open.push(Container {
            tag,
            classes,
            data,
            pieces: Vec::new(),
            holds_embed: false,
        });
    }

    pub(super) fn close(&mut self, out: &mut String, tag: ContainerTag) {
        if !self.lift {
            write_close(out, tag);
            return;
        }
        self.flush(out);
        let container = self.open.pop().expect("open container");
        debug_assert_eq!(container.tag, tag);
        self.pieces().push(Piece::Container(container));
    }

    pub(super) fn embed(&mut self, out: &mut String, index: u32) {
        self.flush(out);
        self.pieces().push(Piece::Embed(index));
        for container in self.open.iter_mut().rev() {
            if std::mem::replace(&mut container.holds_embed, true) {
                break;
            }
        }
    }

    pub(super) fn finish(mut self, mut out: String) -> Vec<Segment> {
        debug_assert!(self.open.is_empty());
        self.flush(&mut out);
        let mut segments = Vec::new();
        emit(self.root, false, &mut segments);
        segments
            .retain(|segment| !matches!(segment, Segment::Html { html } if html.trim().is_empty()));
        segments
    }
}

/// The segments as one HTML string. Embeds are supplied by the host.
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

// A lifted list needs every item lifted: framework wrappers cannot sit directly
// under ul/ol. All other embed-free containers collapse back into adjacent HTML.
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
