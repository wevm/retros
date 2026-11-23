//! A YAML document as a tree of nodes that keep their byte offsets, so configuration errors point
//! at the right line and column. Built from yaml-rust2's event parser, whose markers count
//! characters, not bytes. Mappings keep their keys in source order and reject a duplicate key.

use yaml_rust2::parser::{Event, MarkedEventReceiver, Parser};
use yaml_rust2::scanner::{Marker, TScalarStyle};

use crate::error::{Code, Error};

/// A node and the byte offset of its first character.
#[derive(Clone, Debug)]
pub(crate) struct Node {
    /// What the node holds.
    pub(crate) kind: Kind,
    /// The byte offset of the node in the source, at the opening quote of a quoted scalar.
    pub(crate) offset: usize,
}

/// The kinds of YAML node.
#[derive(Clone, Debug)]
pub(crate) enum Kind {
    /// Key and value pairs in source order.
    Mapping(Vec<(Node, Node)>),
    /// A scalar's text. A plain scalar can also read as a number, a boolean, or null.
    Scalar {
        /// Whether the scalar is unquoted, which is the only style that reads as a number.
        plain: bool,
        /// The scalar's text after YAML removes quotes and escapes.
        text: String,
    },
    /// Items in source order.
    Sequence(Vec<Node>),
}

impl Node {
    /// Returns the text of a scalar, or `None` for a mapping or sequence.
    pub(crate) fn text(&self) -> Option<&str> {
        match &self.kind {
            Kind::Scalar { text, .. } => Some(text),
            _ => None,
        }
    }

    /// Returns whether the node is a plain scalar that YAML reads as null, such as an empty value.
    pub(crate) fn is_null(&self) -> bool {
        matches!(&self.kind, Kind::Scalar { plain: true, text } if matches!(text.as_str(), "" | "~" | "null" | "Null" | "NULL"))
    }
}

/// Parses one YAML document. Returns `None` for a source without a document, such as one that
/// holds only comments.
///
/// # Errors
///
/// Returns a `yaml` error for YAML syntax, a duplicate key, a tag, an alias, or more than one
/// document.
pub(crate) fn parse(source: &str) -> Result<Option<Node>, Error> {
    let mut builder = Builder {
        documents: Vec::new(),
        error: None,
        offsets: source.char_indices().map(|(offset, _)| offset).collect(),
        source,
        stack: Vec::new(),
    };

    Parser::new_from_str(source)
        .load(&mut builder, true)
        .map_err(|error| {
            Error::at(
                Code::Yaml,
                source,
                builder.offset(*error.marker()),
                error.info().to_owned(),
            )
        })?;

    if let Some(error) = builder.error {
        return Err(error);
    }

    let mut documents = builder.documents.into_iter();
    let document = documents.next();
    if let Some(extra) = documents.next() {
        return Err(Error::at(
            Code::Yaml,
            source,
            extra.offset,
            "expected one YAML document, found more than one".to_owned(),
        ));
    }

    Ok(document.filter(|node| !node.is_null()))
}

/// Builds nodes from parser events. The parser cannot return a receiver's error, so the first
/// error is kept and later events are ignored.
struct Builder<'s> {
    documents: Vec<Node>,
    error: Option<Error>,
    /// The byte offset of each character, to turn a marker's character index into a byte offset.
    offsets: Vec<usize>,
    source: &'s str,
    /// Open mappings and sequences, each with a mapping's key that waits for its value.
    stack: Vec<(Node, Option<Node>)>,
}

impl Builder<'_> {
    fn offset(&self, marker: Marker) -> usize {
        self.offsets
            .get(marker.index())
            .copied()
            .unwrap_or(self.source.len())
    }

    fn fail(&mut self, offset: usize, message: &str) {
        self.error = Some(Error::at(
            Code::Yaml,
            self.source,
            offset,
            message.to_owned(),
        ));
    }

    /// Adds a finished node to the open mapping or sequence, or as a document.
    fn add(&mut self, node: Node) {
        let Some((parent, key)) = self.stack.last_mut() else {
            self.documents.push(node);
            return;
        };

        // The parser marks a block mapping after its first key, so a mapping or a sequence
        // starts at its first child.
        if matches!(&parent.kind, Kind::Mapping(pairs) if pairs.is_empty() && key.is_none())
            || matches!(&parent.kind, Kind::Sequence(items) if items.is_empty())
        {
            parent.offset = node.offset;
        }

        match &mut parent.kind {
            Kind::Sequence(items) => items.push(node),
            Kind::Mapping(pairs) => {
                if let Some(key) = key.take() {
                    pairs.push((key, node));
                    return;
                }
                let duplicate = node
                    .text()
                    .is_some_and(|text| pairs.iter().any(|(other, _)| other.text() == Some(text)));
                if duplicate {
                    let message = format!("duplicate key `{}`", node.text().unwrap_or_default());
                    let offset = node.offset;
                    self.fail(offset, &message);
                    return;
                }
                *key = Some(node);
            }
            Kind::Scalar { .. } => {}
        }
    }
}

impl MarkedEventReceiver for Builder<'_> {
    fn on_event(&mut self, event: Event, marker: Marker) {
        if self.error.is_some() {
            return;
        }

        let offset = self.offset(marker);
        match event {
            Event::Scalar(_, _, _, Some(_))
            | Event::MappingStart(_, Some(_))
            | Event::SequenceStart(_, Some(_)) => {
                self.fail(offset, "YAML tags are not supported");
            }
            Event::Alias(_) => self.fail(offset, "YAML aliases are not supported"),
            Event::Scalar(text, style, _, None) => self.add(Node {
                kind: Kind::Scalar {
                    plain: style == TScalarStyle::Plain,
                    text,
                },
                offset,
            }),
            Event::MappingStart(_, None) => self.stack.push((
                Node {
                    kind: Kind::Mapping(Vec::new()),
                    offset,
                },
                None,
            )),
            Event::SequenceStart(_, None) => self.stack.push((
                Node {
                    kind: Kind::Sequence(Vec::new()),
                    offset,
                },
                None,
            )),
            Event::MappingEnd | Event::SequenceEnd => {
                if let Some((node, _)) = self.stack.pop() {
                    self.add(node);
                }
            }
            Event::Nothing
            | Event::StreamStart
            | Event::StreamEnd
            | Event::DocumentStart
            | Event::DocumentEnd => {}
        }
    }
}
