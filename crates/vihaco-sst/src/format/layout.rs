// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Multi-line layout of one-line printed declarations.
//!
//! A printed declaration is split into a bracket tree: groups (`{…}`, `[…]`,
//! `(…)`) whose comma-separated items are sequences of atoms and nested
//! groups. A group is printed flat (exactly as `Print` wrote it) when it fits
//! and carries no comments; otherwise it is broken one item per line with a
//! trailing comma. Comments found in the source declaration are attached to
//! the corresponding items of the printed tree, matching record items by key
//! so that comments follow fields that `Print` reorders.

use std::ops::Range;

use super::comments::{Comment, user_part};

/// A comma-separated sequence of items, delimited by brackets (or by the
/// whole text, for the root).
#[derive(Debug, Clone)]
pub(crate) struct Group {
    /// The opening bracket, or `None` for the root.
    pub open: Option<char>,
    /// From the opening bracket to the closing one, inclusive.
    pub span: Range<usize>,
    pub items: Vec<Item>,
    /// Trailing comments on the line of the opening bracket.
    pub open_trailing: Vec<String>,
    /// Own-line comments after the last item.
    pub dangling: Vec<String>,
}

/// One item of a group: everything between two commas.
#[derive(Debug, Clone)]
pub(crate) struct Item {
    pub nodes: Vec<Node>,
    pub span: Range<usize>,
    /// Own-line comments printed before the item.
    pub leading: Vec<String>,
    /// User comments printed after the item on its last line.
    pub trailing: Vec<String>,
    /// Generated notes, printed as `;;` lines above the item.
    pub notes: Vec<String>,
}

#[derive(Debug, Clone)]
pub(crate) enum Node {
    Atom(Range<usize>),
    Group(Group),
}

impl Node {
    fn span(&self) -> Range<usize> {
        match self {
            Node::Atom(span) => span.clone(),
            Node::Group(group) => group.span.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Token {
    Open(char),
    Close(char),
    Comma,
    Atom,
}

fn closing(open: char) -> char {
    match open {
        '{' => '}',
        '[' => ']',
        _ => ')',
    }
}

/// Split `text` into tokens, skipping whitespace and the given comment ranges.
fn tokenize(text: &str, comments: &[Range<usize>]) -> Vec<(Token, Range<usize>)> {
    let bytes = text.as_bytes();
    let mut tokens = Vec::new();
    let mut index = 0;
    let mut comments = comments.iter().peekable();
    while index < text.len() {
        if let Some(comment) = comments.peek()
            && comment.start == index
        {
            index = comment.end;
            comments.next();
            continue;
        }
        let ch = text[index..].chars().next().expect("in bounds");
        let start = index;
        match ch {
            c if c.is_whitespace() => index += c.len_utf8(),
            '{' | '[' | '(' => {
                index += 1;
                tokens.push((Token::Open(ch), start..index));
            }
            '}' | ']' | ')' => {
                index += 1;
                tokens.push((Token::Close(ch), start..index));
            }
            ',' => {
                index += 1;
                tokens.push((Token::Comma, start..index));
            }
            '"' => {
                index += 1;
                while index < text.len() {
                    match bytes[index] {
                        b'\\' => index += 2,
                        b'"' => {
                            index += 1;
                            break;
                        }
                        _ => index += 1,
                    }
                }
                index = index.min(text.len());
                tokens.push((Token::Atom, start..index));
            }
            _ => {
                while index < text.len() {
                    let c = text[index..].chars().next().expect("in bounds");
                    if c.is_whitespace()
                        || matches!(c, '{' | '[' | '(' | '}' | ']' | ')' | ',' | '"')
                        || comments
                            .peek()
                            .is_some_and(|comment| comment.start == index)
                    {
                        break;
                    }
                    index += c.len_utf8();
                }
                tokens.push((Token::Atom, start..index));
            }
        }
    }
    tokens
}

fn item(nodes: Vec<Node>) -> Item {
    let span =
        nodes.first().expect("non-empty").span().start..nodes.last().expect("non-empty").span().end;
    Item {
        nodes,
        span,
        leading: Vec::new(),
        trailing: Vec::new(),
        notes: Vec::new(),
    }
}

fn parse_group(
    tokens: &[(Token, Range<usize>)],
    position: &mut usize,
    open: Option<(char, usize)>,
    len: usize,
) -> Option<Group> {
    let mut items = Vec::new();
    let mut nodes = Vec::new();
    loop {
        let Some((token, span)) = tokens.get(*position).cloned() else {
            // Only the root may end at the end of the text.
            open.is_none().then_some(())?;
            if !nodes.is_empty() {
                items.push(item(nodes));
            }
            return Some(Group {
                open: None,
                span: 0..len,
                items,
                open_trailing: Vec::new(),
                dangling: Vec::new(),
            });
        };
        *position += 1;
        match token {
            Token::Atom => nodes.push(Node::Atom(span)),
            Token::Comma => {
                if !nodes.is_empty() {
                    items.push(item(std::mem::take(&mut nodes)));
                }
            }
            Token::Open(c) => {
                let group = parse_group(tokens, position, Some((c, span.start)), len)?;
                nodes.push(Node::Group(group));
            }
            Token::Close(c) => {
                let (open_char, start) = open?;
                if closing(open_char) != c {
                    return None;
                }
                if !nodes.is_empty() {
                    items.push(item(nodes));
                }
                return Some(Group {
                    open: Some(open_char),
                    span: start..span.end,
                    items,
                    open_trailing: Vec::new(),
                    dangling: Vec::new(),
                });
            }
        }
    }
}

/// Build the bracket tree of `text`, ignoring `comments`. `None` if the
/// brackets are unbalanced.
pub(crate) fn parse_tree(text: &str, comments: &[Range<usize>]) -> Option<Group> {
    let tokens = tokenize(text, comments);
    let mut position = 0;
    parse_group(&tokens, &mut position, None, text.len())
}

/// The key of a record item (`key: value` or `"key": value`), if any.
fn key(item: &Item, text: &str) -> Option<String> {
    let mut nodes = item.nodes.iter();
    let Some(Node::Atom(first)) = nodes.next() else {
        return None;
    };
    let first = &text[first.clone()];
    let colon_next = || matches!(nodes.clone().next(), Some(Node::Atom(next)) if text[next.clone()].starts_with(':'));
    if first.starts_with('"') {
        return colon_next().then(|| first.to_owned());
    }
    match first.find(':') {
        Some(0) => None,
        Some(index) => Some(first[..index].to_owned()),
        None => colon_next().then(|| first.to_owned()),
    }
}

/// Unique keys of every item, or `None` if some item has no key.
fn keys(group: &Group, text: &str) -> Option<Vec<String>> {
    let keys: Vec<String> = group
        .items
        .iter()
        .map(|item| key(item, text))
        .collect::<Option<_>>()?;
    let mut sorted = keys.clone();
    sorted.sort();
    sorted.dedup();
    (sorted.len() == keys.len()).then_some(keys)
}

/// The item of `printed` corresponding to item `index` of `source`.
fn match_item(
    source: &Group,
    source_text: &str,
    printed: &Group,
    printed_text: &str,
    index: usize,
) -> Option<usize> {
    if let (Some(source_keys), Some(printed_keys)) =
        (keys(source, source_text), keys(printed, printed_text))
    {
        let mut a = source_keys.clone();
        let mut b = printed_keys.clone();
        a.sort();
        b.sort();
        if a == b {
            return printed_keys
                .iter()
                .position(|key| *key == source_keys[index]);
        }
    }
    (source.items.len() == printed.items.len()).then_some(index)
}

/// The `ordinal`-th nested group of `item`.
fn nested(item: &Item, ordinal: usize) -> Option<&Group> {
    item.nodes
        .iter()
        .filter_map(|node| match node {
            Node::Group(group) => Some(group),
            Node::Atom(_) => None,
        })
        .nth(ordinal)
}

fn nested_mut(item: &mut Item, ordinal: usize) -> Option<&mut Group> {
    item.nodes
        .iter_mut()
        .filter_map(|node| match node {
            Node::Group(group) => Some(group),
            Node::Atom(_) => None,
        })
        .nth(ordinal)
}

/// A step from a group into a nested group: the item index, then the
/// ordinal of the group among that item's nested groups.
type Step = (usize, usize);

/// Where a comment belongs in a tree.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Anchor {
    Leading(Vec<Step>, usize),
    Trailing(Vec<Step>, usize),
    Open(Vec<Step>),
    Dangling(Vec<Step>),
}

fn locate(group: &Group, mut path: Vec<Step>, offset: usize, own_line: bool) -> Anchor {
    for (index, item) in group.items.iter().enumerate() {
        if item.span.start <= offset && offset < item.span.end {
            let groups = item.nodes.iter().filter_map(|node| match node {
                Node::Group(group) => Some(group),
                Node::Atom(_) => None,
            });
            for (ordinal, inner) in groups.enumerate() {
                if inner.span.start < offset && offset < inner.span.end {
                    path.push((index, ordinal));
                    return locate(inner, path, offset, own_line);
                }
            }
            return if own_line {
                Anchor::Leading(path, index)
            } else {
                Anchor::Trailing(path, index)
            };
        }
    }
    if own_line {
        match group.items.iter().position(|item| item.span.start > offset) {
            Some(index) => Anchor::Leading(path, index),
            None => Anchor::Dangling(path),
        }
    } else {
        match group.items.iter().rposition(|item| item.span.end <= offset) {
            Some(index) => Anchor::Trailing(path, index),
            None => Anchor::Open(path),
        }
    }
}

/// Where the comments of a source declaration go in its printed tree.
#[derive(Debug, Default)]
pub(crate) struct Placement {
    /// Own-line comments to print before the declaration.
    pub leading: Vec<String>,
    /// Comments to print at the end of the declaration's last line.
    pub trailing: Vec<String>,
}

/// Attach the comments found inside a source declaration to `printed`.
///
/// `comments` carry offsets relative to `source_text`. Comments that cannot
/// be matched to a printed item are returned as declaration-level comments.
pub(crate) fn attach_comments(
    source: &Group,
    source_text: &str,
    printed: &mut Group,
    printed_text: &str,
    comments: &[Comment],
) -> Placement {
    let mut placement = Placement::default();
    for comment in comments {
        let anchor = locate(source, Vec::new(), comment.start, comment.own_line);
        let text = comment.text.clone();
        let path = match &anchor {
            Anchor::Leading(path, _)
            | Anchor::Trailing(path, _)
            | Anchor::Open(path)
            | Anchor::Dangling(path) => path,
        };
        if path.is_empty() {
            // Root level: the declaration itself.
            match anchor {
                Anchor::Leading(..) => placement.leading.push(text),
                _ => placement.trailing.push(text),
            }
            continue;
        }
        let Some((source_group, printed_group)) =
            follow(source, source_text, printed, printed_text, path)
        else {
            placement.leading.push(text);
            continue;
        };
        let item_index = |index: usize| {
            match_item(
                source_group,
                source_text,
                printed_group,
                printed_text,
                index,
            )
        };
        // `Some(None)`: the anchor is the group itself, not one of its items.
        let target = match &anchor {
            Anchor::Leading(_, index) | Anchor::Trailing(_, index) => item_index(*index).map(Some),
            _ => Some(None),
        };
        let Some(target) = target else {
            placement.leading.push(text);
            continue;
        };
        let group =
            follow_mut(source, source_text, printed, printed_text, path).expect("followed above");
        match (anchor, target) {
            (Anchor::Leading(..), Some(index)) => group.items[index].leading.push(text),
            (Anchor::Trailing(..), Some(index)) => {
                let item = &mut group.items[index];
                if item.trailing.is_empty() {
                    item.trailing.push(text);
                } else {
                    // One trailing comment per line; later ones move above.
                    item.leading.push(text);
                }
            }
            (Anchor::Open(_), _) => group.open_trailing.push(text),
            (_, _) => group.dangling.push(text),
        }
    }
    placement
}

/// The source and printed groups at `path`, if the trees correspond there.
fn follow<'a, 'b>(
    source: &'a Group,
    source_text: &str,
    printed: &'b Group,
    printed_text: &str,
    path: &[Step],
) -> Option<(&'a Group, &'b Group)> {
    let (mut source, mut printed) = (source, printed);
    for &(index, ordinal) in path {
        let mapped = match_item(source, source_text, printed, printed_text, index)?;
        let next_source = nested(&source.items[index], ordinal)?;
        let next_printed = nested(&printed.items[mapped], ordinal)?;
        if next_source.open != next_printed.open {
            return None;
        }
        source = next_source;
        printed = next_printed;
    }
    Some((source, printed))
}

fn follow_mut<'b>(
    source: &Group,
    source_text: &str,
    printed: &'b mut Group,
    printed_text: &str,
    path: &[Step],
) -> Option<&'b mut Group> {
    let mut source = source;
    let mut printed = printed;
    for &(index, ordinal) in path {
        let mapped = match_item(source, source_text, printed, printed_text, index)?;
        source = nested(&source.items[index], ordinal)?;
        printed = nested_mut(&mut printed.items[mapped], ordinal)?;
    }
    Some(printed)
}

/// One output line: indentation, code, and trailing comments.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Line {
    pub indent: usize,
    pub code: String,
    /// User comments after the code.
    pub trailing: Vec<String>,
    /// Generated notes, printed as one `;;` line each above the code.
    pub notes: Vec<String>,
}

impl Line {
    pub fn new(indent: usize, code: impl Into<String>) -> Self {
        Self {
            indent,
            code: code.into(),
            trailing: Vec::new(),
            notes: Vec::new(),
        }
    }

    /// Width of the indentation and code, in characters.
    pub fn width(&self) -> usize {
        self.indent + self.code.chars().count()
    }

    pub fn has_comment(&self) -> bool {
        !self.trailing.is_empty()
    }
}

/// Indentation added per nesting level.
pub(crate) const INDENT: usize = 4;

/// Keep the source line breaks so inline `;;` comments stay with code.
pub(crate) fn preserve_lines(
    text: &str,
    comments: &[Range<usize>],
    source_indent: usize,
    indent: usize,
) -> Vec<Line> {
    let mut comments = comments.iter().peekable();
    let mut lines = Vec::new();
    let mut end = 0;
    for line in text.split_inclusive('\n') {
        let start = end;
        end += line.len();
        let (code, comment) = if let Some(range) = comments.next_if(|range| range.start < end) {
            let code = &line[..range.start - start];
            (
                code,
                user_part(&text[range.clone()], code.trim().is_empty()),
            )
        } else {
            (line, None)
        };
        let extra_indent = code
            .chars()
            .take_while(|c| c.is_whitespace())
            .count()
            .saturating_sub(source_indent);
        let indent = indent + extra_indent;
        let code = code.trim();
        if code.is_empty() {
            if let Some(comment) = comment {
                lines.push(Line::new(indent, comment));
            }
        } else {
            let mut line = Line::new(indent, code);
            line.trailing.extend(comment);
            lines.push(line);
        }
    }
    lines
}

fn must_break(group: &Group) -> bool {
    !group.open_trailing.is_empty()
        || !group.dangling.is_empty()
        || group.items.iter().any(|item| {
            !item.leading.is_empty()
                || !item.trailing.is_empty()
                || !item.notes.is_empty()
                || item.nodes.iter().any(|node| match node {
                    Node::Group(group) => must_break(group),
                    Node::Atom(_) => false,
                })
        })
}

/// The layout could not be produced, e.g. a tuple would need a line break.
#[derive(Debug)]
pub(crate) struct Unbreakable;

struct Renderer<'a> {
    text: &'a str,
    max_width: usize,
    lines: Vec<Line>,
    current: Line,
}

impl Renderer<'_> {
    fn push(&mut self, text: &str) {
        self.current.code.push_str(text);
    }

    fn flush(&mut self, next_indent: usize) {
        let line = std::mem::replace(&mut self.current, Line::new(next_indent, ""));
        self.lines.push(line);
    }

    fn space_before(&self, item: &Item, index: usize) -> bool {
        if index == 0 {
            return false;
        }
        let start = item.nodes[index].span().start;
        self.text[..start]
            .chars()
            .next_back()
            .is_some_and(char::is_whitespace)
    }

    fn nodes(&mut self, item: &Item, indent: usize, suffix: usize) -> Result<(), Unbreakable> {
        for (index, node) in item.nodes.iter().enumerate() {
            if self.space_before(item, index) {
                self.push(" ");
            }
            match node {
                Node::Atom(span) => {
                    let atom = &self.text[span.clone()];
                    self.push(atom);
                }
                Node::Group(group) => {
                    let rest = self.text[group.span.start..item.span.end].chars().count() + suffix;
                    let breaks = must_break(group);
                    // An empty group gains nothing from breaking.
                    let fits =
                        self.current.width() + rest <= self.max_width || group.items.is_empty();
                    if group.open == Some('(') {
                        if breaks {
                            return Err(Unbreakable);
                        }
                        let flat = &self.text[group.span.clone()];
                        self.push(flat);
                    } else if !breaks && fits {
                        let flat = &self.text[group.span.clone()];
                        self.push(flat);
                    } else {
                        self.broken(group, indent)?;
                    }
                }
            }
        }
        Ok(())
    }

    fn broken(&mut self, group: &Group, indent: usize) -> Result<(), Unbreakable> {
        let open = group.open.expect("only bracketed groups break");
        self.push(&open.to_string());
        self.current
            .trailing
            .extend(group.open_trailing.iter().cloned());
        let inner = indent + INDENT;
        self.flush(inner);
        for item in &group.items {
            for comment in &item.leading {
                self.lines.push(Line::new(inner, comment.clone()));
            }
            // Notes go above the item's first line.
            self.current.notes.extend(item.notes.iter().cloned());
            self.nodes(item, inner, 1)?;
            self.push(",");
            self.current.trailing.extend(item.trailing.iter().cloned());
            self.flush(inner);
        }
        for comment in &group.dangling {
            self.lines.push(Line::new(inner, comment.clone()));
        }
        self.current = Line::new(indent, closing(open).to_string());
        Ok(())
    }
}

/// Lay out the printed declaration `text` with tree `root` at `indent`.
pub(crate) fn render(
    root: &Group,
    text: &str,
    indent: usize,
    max_width: usize,
) -> Result<Vec<Line>, Unbreakable> {
    let mut renderer = Renderer {
        text,
        max_width,
        lines: Vec::new(),
        current: Line::new(indent, ""),
    };
    for (index, item) in root.items.iter().enumerate() {
        if index > 0 {
            let between = &text[root.items[index - 1].span.end..item.span.start];
            renderer.push(between);
        }
        let suffix = if index + 1 == root.items.len() { 1 } else { 0 };
        renderer.nodes(item, indent, suffix)?;
    }
    renderer.flush(indent);
    Ok(renderer.lines)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout(text: &str, width: usize) -> String {
        let tree = parse_tree(text, &[]).unwrap();
        render(&tree, text, 0, width)
            .unwrap()
            .iter()
            .map(|line| format!("{}{}\n", " ".repeat(line.indent), line.code))
            .collect()
    }

    #[test]
    fn groups_stay_flat_when_they_fit() {
        assert_eq!(
            layout("a { b: [1, 2], c: (1, 2) }", 80),
            "a { b: [1, 2], c: (1, 2) }\n"
        );
    }

    #[test]
    fn groups_break_one_item_per_line_when_too_wide() {
        assert_eq!(
            layout("a { b: [1, 2], c: \"x\" }", 16),
            "a {\n    b: [1, 2],\n    c: \"x\",\n}\n"
        );
        assert_eq!(
            layout("a { b: [111, 222] }", 12),
            "a {\n    b: [\n        111,\n        222,\n    ],\n}\n"
        );
    }

    #[test]
    fn record_keys_match_reordered_fields() {
        let source = "{ b: 1, ; on b\n a: 2 }";
        let comment_start = source.find(';').unwrap();
        let comments = [Comment {
            text: "; on b".to_owned(),
            start: comment_start,
            own_line: false,
            blank_before: false,
        }];
        let comment = comment_start..comment_start + 6;
        let source_tree = parse_tree(source, std::slice::from_ref(&comment)).unwrap();
        let printed = "{ a: 2, b: 1 }";
        let mut printed_tree = parse_tree(printed, &[]).unwrap();
        let placement =
            attach_comments(&source_tree, source, &mut printed_tree, printed, &comments);
        assert!(placement.leading.is_empty() && placement.trailing.is_empty());
        let Node::Group(record) = &printed_tree.items[0].nodes[0] else {
            panic!()
        };
        assert_eq!(record.items[1].trailing, ["; on b"]);
    }
}
