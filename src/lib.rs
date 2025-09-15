//! Replace json comments and trailing commas in place.
//!
//! A fork of a fork:
//!
//! * <https://github.com/tmccombs/json-comments-rs>
//! * <https://github.com/parcel-bundler/parcel/pull/9032>
//!
//! `json-strip-comments` is a library to strip out comments from JSON. By processing text
//! through a [`StripComments`] adapter first, it is possible to use a standard JSON parser (such
//! as [serde_json](https://crates.io/crates/serde_json) with quasi-json input that contains
//! comments.
//!
//! In fact, this code makes few assumptions about the input and could probably be used to strip
//! comments out of other types of code as well, provided that strings use double quotes and
//! backslashes are used for escapes in strings.
//!
//! The following types of comments are supported:
//!   - C style block comments (`/* ... */`)
//!   - C style line comments (`// ...`)
//!   - Shell style line comments (`# ...`)
//!
//! ## Example
//!
//! ```rust
#![doc = include_str!("../examples/example.rs")]
//! ```

use std::io::{ErrorKind, Read, Result};

#[derive(Eq, PartialEq, Copy, Clone, Debug)]
#[repr(u8)]
enum State {
    Top,
    InString,
    StringEscape,
    InComment,
    InBlockComment,
    MaybeCommentEnd,
    InLineComment,
}

use State::{
    InBlockComment, InComment, InLineComment, InString, MaybeCommentEnd, StringEscape, Top,
};

/// A [`Read`] that transforms another [`Read`] so that it changes all comments to spaces so that a downstream json parser
/// (such as json-serde) doesn't choke on them.
///
/// The supported comments are:
///   - C style block comments (`/* ... */`)
///   - C style line comments (`// ...`)
///   - Shell style line comments (`# ...`)
///
/// ## Example
/// ```
/// use json_strip_comments::StripComments;
/// use std::io::Read;
///
/// let input = r#"{
/// // c line comment
/// "a": "comment in string /* a */",
/// ## shell line comment
/// } /** end */"#;
///
/// let mut stripped = String::new();
/// StripComments::new(input.as_bytes()).read_to_string(&mut stripped).unwrap();
///
/// assert_eq!(stripped, "{
///                  \n\"a\": \"comment in string /* a */\",
///                     \n}           ");
///
/// ```
///
pub struct StripComments<T: Read> {
    inner: T,
    state: State,
}

impl<T> StripComments<T>
where
    T: Read,
{
    pub fn new(input: T) -> Self {
        Self {
            inner: input,
            state: Top,
        }
    }
}

impl<T> Read for StripComments<T>
where
    T: Read,
{
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        let count = self.inner.read(buf)?;
        if count > 0 {
            strip_buf(&mut self.state, &mut buf[..count])?;
        } else if self.state != Top && self.state != InLineComment {
            return Err(ErrorKind::InvalidData.into());
        }
        Ok(count)
    }
}

/// Strips comments from a string in place, replacing it with whitespaces.
///
/// /// ## Example
/// ```
/// use json_strip_comments::strip_comments_in_place;
///
/// let mut string = String::from(r#"{
/// // c line comment
/// "a": "comment in string /* a */"
/// ## shell line comment
/// } /** end */"#);
///
/// strip_comments_in_place(&mut string).unwrap();
///
/// assert_eq!(string, "{
///                  \n\"a\": \"comment in string /* a */\"
///                     \n}           ");
///
/// ```
pub fn strip_comments_in_place(s: &mut str) -> Result<()> {
    // Safety: we have made sure the text is UTF-8
    strip_buf(&mut Top, unsafe { s.as_bytes_mut() })
}

pub fn strip(s: &mut str) -> Result<()> {
    strip_comments_in_place(s)
}


fn consume_comment_whitespace_until_maybe_bracket(
    state: &mut State,
    buf: &mut [u8],
    i: &mut usize,
) -> Result<bool> {
    *i += 1;
    let len = buf.len();
    while *i < len {
        let c = &mut buf[*i];
        *state = match state {
            Top => {
                *state = top(c);
                if c.is_ascii_whitespace() {
                    *i += 1;
                    continue;
                }
                return Ok(*c == b'}' || *c == b']');
            }
            InString => in_string(*c),
            StringEscape => InString,
            InComment => in_comment(c)?,
            InBlockComment => consume_block_comments(buf, i),
            MaybeCommentEnd => maybe_comment_end(c),
            InLineComment => consume_line_comments(buf, i),
        };
        *i += 1;
    }
    Ok(false)
}

fn strip_buf(state: &mut State, buf: &mut [u8]) -> Result<()> {
    let mut i = 0;
    let len = buf.len();
    
    // Fast path for Top state which is most common
    while i < len {
        let c = &mut buf[i];
        
        match state {
            Top => {
                let cur = i;
                let new_state = top(c);
                if *c == b',' {
                    let mut temp_state = new_state;
                    if consume_comment_whitespace_until_maybe_bracket(&mut temp_state, buf, &mut i)? {
                        buf[cur] = b' ';
                    }
                    *state = temp_state;
                } else {
                    *state = new_state;
                }
            }
            InString => *state = in_string(*c),
            StringEscape => *state = InString,
            InComment => *state = in_comment(c)?,
            InBlockComment => *state = consume_block_comments(buf, &mut i),
            MaybeCommentEnd => *state = maybe_comment_end(c),
            InLineComment => *state = consume_line_comments(buf, &mut i),
        }
        
        i += 1;
    }
    Ok(())
}

#[inline(always)]
fn consume_line_comments(buf: &mut [u8], i: &mut usize) -> State {
    let cur = *i;
    let remaining = &buf[*i..];
    match memchr::memchr(b'\n', remaining) {
        Some(offset) => {
            *i += offset;
            buf[cur..*i].fill(b' ');
            Top
        }
        None => {
            let len = buf.len();
            *i = len - 1;
            buf[cur..len].fill(b' ');
            InLineComment
        }
    }
}

#[inline(always)]
fn consume_block_comments(buf: &mut [u8], i: &mut usize) -> State {
    let cur = *i;
    let remaining = &buf[*i..];
    match memchr::memchr(b'*', remaining) {
        Some(offset) => {
            *i += offset;
            buf[cur..=*i].fill(b' ');
            MaybeCommentEnd
        }
        None => {
            let len = buf.len();
            *i = len - 1;
            buf[cur..len].fill(b' ');
            InBlockComment
        }
    }
}

#[inline(always)]
fn top(c: &mut u8) -> State {
    match *c {
        b'"' => InString,
        b'/' => {
            *c = b' ';
            InComment
        }
        b'#' => {
            *c = b' ';
            InLineComment
        }
        _ => Top,
    }
}

#[inline(always)]
fn in_string(c: u8) -> State {
    match c {
        b'"' => Top,
        b'\\' => StringEscape,
        _ => InString,
    }
}

#[inline]
fn in_comment(c: &mut u8) -> Result<State> {
    let new_state = match *c {
        b'*' => InBlockComment,
        b'/' => InLineComment,
        _ => return Err(ErrorKind::InvalidData.into()),
    };
    *c = b' ';
    Ok(new_state)
}

#[inline]
fn maybe_comment_end(c: &mut u8) -> State {
    let old = *c;
    *c = b' ';
    match old {
        b'/' => Top,
        b'*' => MaybeCommentEnd,
        _ => InBlockComment,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{ErrorKind, Read};

    fn strip_string(input: &str) -> String {
        let mut out = String::new();
        let count = StripComments::new(input.as_bytes())
            .read_to_string(&mut out)
            .unwrap();
        assert_eq!(count, input.len());
        out
    }

    #[test]
    fn block_comments() {
        let json = r#"{/* Comment */"hi": /** abc */ "bye"}"#;
        let stripped = strip_string(json);
        assert_eq!(stripped, r#"{             "hi":            "bye"}"#);
    }

    #[test]
    fn block_comments_with_possible_end() {
        let json = r#"{/* Comment*PossibleEnd */"hi": /** abc */ "bye"}"#;
        let stripped = strip_string(json);
        assert_eq!(
            stripped,
            r#"{                         "hi":            "bye"}"#
        );
    }

    // See https://github.com/tmccombs/json-comments-rs/issues/12
    // Make sure we can parse a block comment that ends with more than one "*"
    #[test]
    fn doc_comment() {
        let json = r##"/** C **/ { "foo": 123 }"##;
        let stripped = strip_string(json);
        assert_eq!(stripped, r##"          { "foo": 123 }"##);
    }

    #[test]
    fn line_comments() {
        let json = r#"{
            // line comment
            "a": 4,
            # another
        }"#;

        let expected = "{
                           \n            \"a\": 4,
                     \n        }";

        assert_eq!(strip_string(json), expected);
    }

    #[test]
    fn incomplete_string() {
        let json = r#""foo"#;
        let mut stripped = String::new();

        let err = StripComments::new(json.as_bytes())
            .read_to_string(&mut stripped)
            .unwrap_err();
        assert_eq!(err.kind(), ErrorKind::InvalidData);
    }

    #[test]
    fn incomplete_comment() {
        let json = "/* foo ";
        let mut stripped = String::new();

        let err = StripComments::new(json.as_bytes())
            .read_to_string(&mut stripped)
            .unwrap_err();
        assert_eq!(err.kind(), ErrorKind::InvalidData);
    }

    #[test]
    fn incomplete_comment2() {
        let json = "/* foo *";
        let mut stripped = String::new();

        let err = StripComments::new(json.as_bytes())
            .read_to_string(&mut stripped)
            .unwrap_err();
        assert_eq!(err.kind(), ErrorKind::InvalidData);
    }


    #[test]
    fn strip_in_place() {
        let mut json = String::from(r#"{/* Comment */"hi": /** abc */ "bye"}"#);
        strip_comments_in_place(&mut json).unwrap();
        assert_eq!(json, r#"{             "hi":            "bye"}"#);
    }

    #[test]
    fn trailing_comma() {
        let mut json = String::from(
            r#"{
            "a1": [1,],
            "a2": [1,/* x */],
            "a3": [
                1, // x
            ],
            "o1": {v:1,},
            "o2": {v:1,/* x */},
            "o3": {
                "v":1, // x
            },
            # another
        }"#,
        );
        strip_comments_in_place(&mut json).unwrap();

        let expected = r#"{
            "a1": [1 ],
            "a2": [1        ],
            "a3": [
                1
            ],
            "o1": {v:1 },
            "o2": {v:1        },
            "o3": {
                "v":1
            }
        }"#;

        assert_eq!(
            json.replace(|s: char| s.is_ascii_whitespace(), ""),
            expected.replace(|s: char| s.is_ascii_whitespace(), "")
        );
    }
}
