// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Splitting a block into statements: the line rules of SST.
//!
//! A statement ends at the end of its line. It continues onto the next line
//! only while a `(`, `[`, or `{` is open, like implicit line joining in
//! Python; there is no `\` continuation and no statement terminator. A comment
//! runs from `;` to the end of the line, except inside a `"…"` string literal.
//! Indentation and spacing carry no meaning, and `\r\n` is a newline.
//!
//! [`statements`] is a pre-pass: it finds each statement's span in the block
//! and leaves the statement text untouched, so the derived parsers run over a
//! slice of the original text and every span they report is a block offset.
//! A comment inside a bracket-continued statement stays in its slice, where
//! the trivia of collections and records skips it.

use std::ops::Range;

use crate::spanned::SyntaxError;

/// One statement of a block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Statement {
    /// From the first character of the statement to its last character that
    /// is not whitespace or a comment, relative to the block.
    pub span: Range<usize>,
    /// Whether the statement's brackets are balanced. An unbalanced statement
    /// has already been reported in [`Statements::errors`] and should not be
    /// parsed.
    pub balanced: bool,
}

/// The statements of a block and its bracket errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Statements {
    pub statements: Vec<Statement>,
    /// One error per unclosed bracket, at the bracket.
    pub errors: Vec<SyntaxError>,
}

fn closer(open: char) -> char {
    match open {
        '(' => ')',
        '[' => ']',
        _ => '}',
    }
}

/// The offset of the end of the string literal whose opening `"` is at
/// `start`: just past its closing `"`, or the end of the line if it is not
/// closed on its line.
fn string_end(text: &str, start: usize) -> usize {
    let bytes = text.as_bytes();
    let mut index = start + 1;
    while index < bytes.len() {
        match bytes[index] {
            b'\\' if bytes.get(index + 1).is_some_and(|b| *b != b'\n') => index += 2,
            b'"' => return index + 1,
            b'\n' => return index,
            _ => index += 1,
        }
    }
    bytes.len()
}

/// The end of the physical line containing `offset`, excluding the `\n`.
fn line_end(text: &str, offset: usize) -> usize {
    text[offset..]
        .find('\n')
        .map_or(text.len(), |index| offset + index)
}

/// Split `text` into statements. `first_line` is the file line of the start
/// of `text`, used in error messages.
///
/// An unclosed bracket is reported as ``unclosed `[` opened at line N``; its
/// statement is cut at the end of the bracket's line and marked unbalanced,
/// and splitting resumes on the next line, so the rest of the block is
/// unaffected. A closing bracket that matches no open bracket is left to the
/// statement's parser.
pub fn statements(text: &str, first_line: usize) -> Statements {
    let line_of = |offset: usize| first_line + text[..offset].matches('\n').count();
    let mut out = Statements {
        statements: Vec::new(),
        errors: Vec::new(),
    };
    let mut position = 0;
    while position < text.len() {
        // One statement, starting on the physical line at `position`.
        let mut open: Vec<(char, usize)> = Vec::new();
        let mut start = None;
        let mut end = position;
        let mut balanced = true;
        let mut index = position;
        let mut next = text.len();
        while index < text.len() {
            let c = text[index..].chars().next().expect("in bounds");
            match c {
                '\n' if open.is_empty() => {
                    next = index + 1;
                    break;
                }
                ';' => {
                    index = line_end(text, index);
                    continue;
                }
                c if c.is_whitespace() => {
                    index += c.len_utf8();
                    continue;
                }
                '"' => {
                    start.get_or_insert(index);
                    index = string_end(text, index);
                    end = index;
                    continue;
                }
                '(' | '[' | '{' => open.push((c, index)),
                ')' | ']' | '}' => {
                    if let Some(found) = open.iter().rposition(|(open, _)| closer(*open) == c) {
                        // Brackets opened inside this pair and never closed.
                        for &(bracket, at) in &open[found + 1..] {
                            out.errors.push(unclosed(bracket, at, line_of(at)));
                            balanced = false;
                        }
                        open.truncate(found);
                    }
                }
                _ => {}
            }
            start.get_or_insert(index);
            index += c.len_utf8();
            end = index;
        }
        if let Some(&(bracket, at)) = open.first() {
            // The block ended inside a bracket: report the outermost one, cut
            // the statement at the end of its line, and resume after it.
            let cut = line_end(text, at);
            // Lines after the cut are split again; forget what they reported.
            out.errors.retain(|error| error.span.start < cut);
            out.errors.push(unclosed(bracket, at, line_of(at)));
            end = trim_comment_end(text, start.unwrap_or(at), cut);
            balanced = false;
            next = (cut + 1).min(text.len());
        }
        if let Some(start) = start {
            out.statements.push(Statement {
                span: start..end.max(start),
                balanced,
            });
        }
        position = next;
    }
    out.errors.sort_by_key(|error| error.span.start);
    out
}

/// The end of the code between `start` and `limit` on one line: before any
/// comment and trailing whitespace.
fn trim_comment_end(text: &str, start: usize, limit: usize) -> usize {
    let mut index = start;
    let mut end = start;
    while index < limit {
        let c = text[index..].chars().next().expect("in bounds");
        match c {
            ';' => break,
            '"' => {
                index = string_end(text, index).min(limit);
                end = index;
            }
            c if c.is_whitespace() => index += c.len_utf8(),
            c => {
                index += c.len_utf8();
                end = index;
            }
        }
    }
    end
}

fn unclosed(bracket: char, at: usize, line: usize) -> SyntaxError {
    SyntaxError {
        span: at..at + 1,
        message: format!("unclosed `{bracket}` opened at line {line}"),
        label: format!("this `{bracket}` is never closed"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn split(text: &str) -> Vec<&str> {
        statements(text, 1)
            .statements
            .iter()
            .map(|statement| &text[statement.span.clone()])
            .collect()
    }

    #[test]
    fn statements_end_at_the_end_of_the_line() {
        assert_eq!(
            split("  a 1  ; one\n\n\tb ; two\r\n; only a comment\nc"),
            ["a 1", "b", "c"]
        );
    }

    #[test]
    fn brackets_continue_a_statement() {
        let text = "x [1, ; first\n  2,\n] y\nz (\n)\n";
        assert_eq!(split(text), ["x [1, ; first\n  2,\n] y", "z (\n)"]);
    }

    #[test]
    fn semicolons_in_strings_are_not_comments() {
        assert_eq!(split("p \"a ; b\" ; c\n"), ["p \"a ; b\""]);
        assert_eq!(split("p \"[\" q\nr"), ["p \"[\" q", "r"]);
    }

    #[test]
    fn unclosed_brackets_are_reported_and_splitting_recovers() {
        let text = "a\nb [1,\nc\nd\n";
        let split = statements(text, 10);
        let spans: Vec<_> = split
            .statements
            .iter()
            .map(|statement| (&text[statement.span.clone()], statement.balanced))
            .collect();
        assert_eq!(
            spans,
            [("a", true), ("b [1,", false), ("c", true), ("d", true)]
        );
        assert_eq!(split.errors.len(), 1);
        assert_eq!(split.errors[0].message, "unclosed `[` opened at line 11");
        assert_eq!(split.errors[0].span, 4..5);
    }

    #[test]
    fn mismatched_closers_report_the_inner_bracket() {
        let split = statements("a { [ }\nb\n", 1);
        assert_eq!(split.errors[0].message, "unclosed `[` opened at line 1");
        assert!(!split.statements[0].balanced);
        assert!(split.statements[1].balanced);
    }
}
