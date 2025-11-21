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

use State::{InBlockComment, InComment, InLineComment, InString, MaybeCommentEnd, StringEscape, Top};

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
        Self { inner: input, state: Top }
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
#[inline]
pub fn strip_comments_in_place(s: &mut str) -> Result<()> {
    // Safety: we have made sure the text is UTF-8
    strip_buf(&mut Top, unsafe { s.as_bytes_mut() })
}

#[inline]
pub fn strip(s: &mut str) -> Result<()> {
    strip_comments_in_place(s)
}

#[inline]
pub fn strip_slice(s: &mut [u8]) -> Result<()> {
    strip_buf(&mut Top, s)
}

fn strip_buf(state: &mut State, buf: &mut [u8]) -> Result<()> {
    let mut i = 0;
    let len = buf.len();
    let mut pending_comma_pos: Option<usize> = None;

    while i < len {
        let c = &mut buf[i];

        match *state {
            Top => {
                match *c {
                    b'"' => *state = InString,
                    b'/' => {
                        *c = b' ';
                        *state = InComment;
                    }
                    b'#' => {
                        *c = b' ';
                        *state = InLineComment;
                    }
                    b',' => {
                        pending_comma_pos = Some(i);
                    }
                    b'}' | b']' => {
                        if let Some(pos) = pending_comma_pos {
                            buf[pos] = b' ';
                            pending_comma_pos = None;
                        }
                    }
                    _ => {
                        if !c.is_ascii_whitespace() {
                            pending_comma_pos = None;
                        }
                    }
                }
            }
            InString => {
                match *c {
                    b'"' => *state = Top,
                    b'\\' => *state = StringEscape,
                    _ => {}
                }
            }
            StringEscape => *state = InString,
            InComment => {
                let old = *c;
                *c = b' ';
                match old {
                    b'*' => *state = InBlockComment,
                    b'/' => *state = InLineComment,
                    _ => return Err(ErrorKind::InvalidData.into()),
                }
            }
            InBlockComment => {
                let old = *c;
                // Preserve newlines in block comments
                if old != b'\n' && old != b'\r' {
                    *c = b' ';
                }
                if old == b'*' {
                    *state = MaybeCommentEnd;
                }
            }
            MaybeCommentEnd => {
                let old = *c;
                // Preserve newlines in block comments
                if old != b'\n' && old != b'\r' {
                    *c = b' ';
                }
                match old {
                    b'/' => *state = Top,
                    b'*' => *state = MaybeCommentEnd,
                    _ => *state = InBlockComment,
                }
            }
            InLineComment => {
                if *c == b'\n' {
                    *state = Top;
                } else if *c != b'\r' {
                    // Preserve \r as well (for \r\n line endings)
                    *c = b' ';
                }
            }
        }

        i += 1;
    }
    Ok(())
}

