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

/// Strips comments and trailing commas from a string in place, replacing them with whitespace.
///
/// # Errors
///
/// Returns [`ErrorKind::InvalidData`] if a `/` is not followed by `/` or `*`.
///
/// ## Example
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
    // SAFETY: only ASCII bytes are ever replaced, and they are replaced with ASCII spaces.
    // A UTF-8 multi-byte sequence never contains ASCII bytes, so the result is valid UTF-8.
    strip_buf(&mut Top, unsafe { s.as_bytes_mut() })
}

/// Alias of [`strip_comments_in_place`].
///
/// # Errors
///
/// See [`strip_comments_in_place`].
#[inline]
pub fn strip(s: &mut str) -> Result<()> {
    strip_comments_in_place(s)
}

/// Strips comments and trailing commas from a byte slice in place, replacing them with whitespace.
///
/// # Errors
///
/// See [`strip_comments_in_place`].
#[inline]
pub fn strip_slice(s: &mut [u8]) -> Result<()> {
    strip_buf(&mut Top, s)
}

/// Bytes that end a run of [`Top`] state bytes.
static TOP_SPECIAL: [bool; 256] = {
    let mut table = [false; 256];
    table[b'"' as usize] = true;
    table[b'/' as usize] = true;
    table[b'#' as usize] = true;
    table[b',' as usize] = true;
    table
};

fn strip_buf(state: &mut State, buf: &mut [u8]) -> Result<()> {
    let len = buf.len();
    let mut i = 0;
    let mut s = *state;
    // Position of a comma that is followed only by whitespace and comments so far.
    // It is blanked out if the next significant byte is `}` or `]`.
    let mut pending_comma: Option<usize> = None;

    while i < len {
        match s {
            Top => {
                if let Some(comma) = pending_comma {
                    while i < len && buf[i].is_ascii_whitespace() {
                        i += 1;
                    }
                    if i == len {
                        break;
                    }
                    match buf[i] {
                        b'}' | b']' => {
                            buf[comma] = b' ';
                            pending_comma = None;
                        }
                        // A comment may sit between the comma and the closing bracket.
                        b'/' | b'#' => {}
                        // A comma right after a pending comma is not tracked itself,
                        // so `[1,,]` is left unchanged (same as previous versions).
                        b',' => {
                            pending_comma = None;
                            i += 1;
                            continue;
                        }
                        _ => pending_comma = None,
                    }
                }
                while i < len && !TOP_SPECIAL[buf[i] as usize] {
                    i += 1;
                }
                if i == len {
                    break;
                }
                match buf[i] {
                    b'"' => s = InString,
                    b'/' => {
                        buf[i] = b' ';
                        s = InComment;
                    }
                    b'#' => {
                        buf[i] = b' ';
                        s = InLineComment;
                    }
                    _ => pending_comma = Some(i),
                }
                i += 1;
            }
            InString => match memchr::memchr2(b'"', b'\\', &buf[i..]) {
                Some(offset) => {
                    i += offset;
                    s = if buf[i] == b'"' { Top } else { StringEscape };
                    i += 1;
                }
                None => i = len,
            },
            StringEscape => {
                s = InString;
                i += 1;
            }
            InComment => {
                s = match buf[i] {
                    b'*' => InBlockComment,
                    b'/' => InLineComment,
                    _ => {
                        *state = s;
                        return Err(ErrorKind::InvalidData.into());
                    }
                };
                buf[i] = b' ';
                i += 1;
            }
            InBlockComment => match memchr::memchr(b'*', &buf[i..]) {
                Some(offset) => {
                    let end = i + offset;
                    buf[i..=end].fill(b' ');
                    i = end + 1;
                    s = MaybeCommentEnd;
                }
                None => {
                    buf[i..].fill(b' ');
                    i = len;
                }
            },
            MaybeCommentEnd => {
                s = match buf[i] {
                    b'/' => Top,
                    b'*' => MaybeCommentEnd,
                    _ => InBlockComment,
                };
                buf[i] = b' ';
                i += 1;
            }
            InLineComment => match memchr::memchr(b'\n', &buf[i..]) {
                Some(offset) => {
                    let end = i + offset;
                    buf[i..end].fill(b' ');
                    // The newline itself is kept and handled in `Top`.
                    i = end;
                    s = Top;
                }
                None => {
                    buf[i..].fill(b' ');
                    i = len;
                }
            },
        }
    }

    *state = s;
    Ok(())
}
