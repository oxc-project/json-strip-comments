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
    Top = 0,
    InString = 1,
    StringEscape = 2,
    InComment = 3,
    InBlockComment = 4,
    MaybeCommentEnd = 5,
    InLineComment = 6,
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
        // Use discriminant comparison for better branch prediction
        *state = match *state as u8 {
            0 => { // Top
                *state = top(c);
                if c.is_ascii_whitespace() {
                    *i += 1;
                    continue;
                }
                return Ok(*c == b'}' || *c == b']');
            }
            1 => in_string(*c), // InString
            2 => InString, // StringEscape
            3 => in_comment(c)?, // InComment
            4 => consume_block_comments(buf, i), // InBlockComment
            5 => maybe_comment_end(c), // MaybeCommentEnd
            6 => consume_line_comments(buf, i), // InLineComment
            _ => unsafe { std::hint::unreachable_unchecked() }
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

        // Use discriminant comparison for better branch prediction
        match *state as u8 {
            0 => { // Top
                let cur = i;
                let new_state = top(c);
                if *c == b',' {
                    let mut temp_state = new_state;
                    if consume_comment_whitespace_until_maybe_bracket(&mut temp_state, buf, &mut i)?
                    {
                        buf[cur] = b' ';
                    }
                    *state = temp_state;
                } else {
                    *state = new_state;
                }
            }
            1 => *state = in_string(*c), // InString
            2 => *state = InString, // StringEscape
            3 => *state = in_comment(c)?, // InComment
            4 => *state = consume_block_comments(buf, &mut i), // InBlockComment
            5 => *state = maybe_comment_end(c), // MaybeCommentEnd
            6 => *state = consume_line_comments(buf, &mut i), // InLineComment
            _ => unsafe { std::hint::unreachable_unchecked() }
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
    // Most common case: not a special character
    if *c != b'"' && *c != b'/' && *c != b'#' {
        return Top;
    }

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
    // Most common case: regular character in string
    if c != b'"' && c != b'\\' {
        return InString;
    }

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
