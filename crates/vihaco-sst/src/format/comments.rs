// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Finding `//` comments in source text and separating user comments from
//! generated `//!` ones.

use std::ops::Range;

/// A user comment found in the source, with its generated part removed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Comment {
    /// The user part, starting with `//` and without trailing whitespace.
    pub text: String,
    /// The file offset of the leading `//`.
    pub start: usize,
    /// Whether only whitespace precedes the comment on its line.
    pub own_line: bool,
    /// Whether a blank line separates the comment from the previous element
    /// of its block. Only meaningful for own-line comments.
    pub blank_before: bool,
}

/// The user part of a raw comment (`//` to end of line), or `None` when the
/// comment is entirely generated.
///
/// `//!` starts a generated comment wherever it appears, so the user part of
/// `// note //! generated` is `// note`.
pub(crate) fn user_part(raw: &str) -> Option<String> {
    let raw = raw.trim_end();
    let body = raw.strip_prefix("//")?;
    if body.starts_with('!') {
        return None;
    }
    let user = match body.find("//!") {
        Some(index) => &raw[..2 + index],
        None => raw,
    };
    Some(user.trim_end().to_owned())
}

/// What a scan of a source range found besides comments.
pub(crate) struct Scan {
    /// Raw comments: from `//` to the end of the line (excluding `\r\n`).
    pub comments: Vec<Range<usize>>,
    /// The first character that is neither whitespace, a comment, nor an
    /// allowed punctuation character, if any.
    pub unexpected: Option<usize>,
}

/// Find the comments in `text[range]`.
///
/// With `any_content` (inside a parsed item) `"…"` string literals are
/// skipped and any other content is accepted. Otherwise (between items) only
/// whitespace, comments, and the `allowed` characters are expected, and the
/// first other character is reported in [`Scan::unexpected`].
pub(crate) fn scan(text: &str, range: Range<usize>, any_content: bool, allowed: &[char]) -> Scan {
    let mut comments = Vec::new();
    let mut unexpected = None;
    let bytes = text.as_bytes();
    let end = range.end.min(text.len());
    let mut index = range.start;
    while index < end {
        let c = bytes[index];
        if c == b'/' && bytes.get(index + 1) == Some(&b'/') && index + 1 < end {
            let line_end = text[index..end]
                .find('\n')
                .map_or(end, |offset| index + offset);
            let comment_end = if text[..line_end].ends_with('\r') {
                line_end - 1
            } else {
                line_end
            };
            comments.push(index..comment_end);
            index = line_end;
        } else if c == b'"' && any_content {
            // Skip the string literal, honouring backslash escapes.
            index += 1;
            while index < end {
                match bytes[index] {
                    b'\\' => index += 2,
                    b'"' => {
                        index += 1;
                        break;
                    }
                    _ => index += 1,
                }
            }
        } else {
            let ch = text[index..].chars().next().expect("in bounds");
            if !ch.is_whitespace() && !any_content && !allowed.contains(&ch) && unexpected.is_none()
            {
                unexpected = Some(index);
            }
            index += ch.len_utf8();
        }
    }
    Scan {
        comments,
        unexpected,
    }
}

/// Whether only whitespace precedes `offset` on its line.
pub(crate) fn starts_line(text: &str, offset: usize) -> bool {
    let line_start = text[..offset].rfind('\n').map_or(0, |index| index + 1);
    text[line_start..offset].trim().is_empty()
}

/// Whether a whitespace-only line lies strictly between `from` and `to`.
pub(crate) fn blank_line_between(text: &str, from: usize, to: usize) -> bool {
    let segment = &text[from.min(to)..to];
    let mut lines = segment.split('\n');
    lines.next();
    let mut lines: Vec<&str> = lines.collect();
    lines.pop();
    lines.iter().any(|line| line.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_parts_drop_generated_comments() {
        assert_eq!(user_part("// hello  "), Some("// hello".to_owned()));
        assert_eq!(user_part("//! generated"), None);
        assert_eq!(
            user_part("// note //! generated"),
            Some("// note".to_owned())
        );
        assert_eq!(user_part("/// doc"), Some("/// doc".to_owned()));
        assert_eq!(user_part("//"), Some("//".to_owned()));
    }

    #[test]
    fn scans_skip_strings_and_report_stray_content() {
        let text = "t.print \"a // b\" // c\r\n; x";
        let found = scan(text, 0..text.len(), true, &[]);
        assert_eq!(found.comments, vec![17..21]);
        let found = scan(text, 21..text.len(), false, &[';']);
        assert_eq!(found.unexpected, Some(25));
    }

    #[test]
    fn blank_lines_are_whitespace_only_lines() {
        let text = "a\n  \n//! x\nb";
        assert!(blank_line_between(text, 1, 11));
        let text = "a\n//! x\nb";
        assert!(!blank_line_between(text, 1, 8));
    }
}
