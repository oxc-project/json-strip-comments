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
    settings: CommentSettings,
}

impl<T> StripComments<T>
where
    T: Read,
{
    pub fn new(input: T) -> Self {
        Self {
            inner: input,
            state: Top,
            settings: CommentSettings::default(),
        }
    }

    /// Create a new `StripComments` with settings which may be different from the default.
    ///
    /// This is useful if you wish to disable allowing certain kinds of comments.
    #[inline]
    pub fn with_settings(settings: CommentSettings, input: T) -> Self {
        Self {
            inner: input,
            state: Top,
            settings,
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
            strip_buf(&mut self.state, &mut buf[..count], self.settings)?;
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
/// use json_strip_comments::{strip_comments_in_place, CommentSettings};
///
/// let mut string = String::from(r#"{
/// // c line comment
/// "a": "comment in string /* a */"
/// ## shell line comment
/// } /** end */"#);
///
/// strip_comments_in_place(&mut string, CommentSettings::default()).unwrap();
///
/// assert_eq!(string, "{
///                  \n\"a\": \"comment in string /* a */\"
///                     \n}           ");
///
/// ```
pub fn strip_comments_in_place(s: &mut str, settings: CommentSettings) -> Result<()> {
    // Safety: we have made sure the text is UTF-8
    strip_buf(&mut Top, unsafe { s.as_bytes_mut() }, settings)
}

pub fn strip(s: &mut str) -> Result<()> {
    strip_comments_in_place(s, CommentSettings::all())
}

/// Settings for `StripComments`
///
/// The default is for all comment types to be enabled.
#[derive(Copy, Clone, Debug)]
pub struct CommentSettings {
    /// True if c-style block comments (`/* ... */`) are removed.
    pub block_comments: bool,
    /// True if c-style `//` line comments are removed.
    pub slash_line_comments: bool,
    /// True if shell-style `#` line comments are removed.
    pub hash_line_comments: bool,
    /// True of trailing commas are removed.
    pub trailing_commas: bool,
}

impl Default for CommentSettings {
    fn default() -> Self {
        Self::all()
    }
}

impl CommentSettings {
    /// Enable all comment Styles
    pub const fn all() -> Self {
        Self {
            block_comments: true,
            slash_line_comments: true,
            hash_line_comments: true,
            trailing_commas: true,
        }
    }
    /// Only allow line comments starting with `#`
    pub const fn hash_only() -> Self {
        Self {
            hash_line_comments: true,
            block_comments: false,
            slash_line_comments: false,
            trailing_commas: false,
        }
    }
    /// Only allow "c-style" comments.
    ///
    /// Specifically, line comments beginning with `//` and
    /// block comment like `/* ... */`.
    pub const fn c_style() -> Self {
        Self {
            block_comments: true,
            slash_line_comments: true,
            hash_line_comments: false,
            trailing_commas: true,
        }
    }

    /// Create a new `StripComments` for `input`, using these settings.
    ///
    /// Transform `input` into a [`Read`] that strips out comments.
    /// The types of comments to support are determined by the configuration of
    /// `self`.
    ///
    /// ## Examples
    ///
    /// ```
    /// use json_strip_comments::CommentSettings;
    /// use std::io::Read;
    ///
    /// let input = r#"{
    /// // c line comment
    /// "a": "b"
    /// /** multi line
    /// comment
    /// */ }"#;
    ///
    /// let mut stripped = String::new();
    /// CommentSettings::c_style().strip_comments(input.as_bytes()).read_to_string(&mut stripped).unwrap();
    ///
    /// assert_eq!(stripped, "{
    ///                  \n\"a\": \"b\"
    ///                           }");
    /// ```
    ///
    /// ```
    /// use json_strip_comments::CommentSettings;
    /// use std::io::Read;
    ///
    /// let input = r#"{
    /// ## shell line comment
    /// "a": "b"
    /// }"#;
    ///
    /// let mut stripped = String::new();
    /// CommentSettings::hash_only().strip_comments(input.as_bytes()).read_to_string(&mut stripped).unwrap();
    ///
    /// assert_eq!(stripped, "{
    ///                     \n\"a\": \"b\"\n}");
    /// ```
    #[inline]
    pub fn strip_comments<I: Read>(self, input: I) -> StripComments<I> {
        StripComments::with_settings(self, input)
    }
}

fn consume_comment_whitespace_until_maybe_bracket(
    state: &mut State,
    buf: &mut [u8],
    i: &mut usize,
    settings: CommentSettings,
) -> Result<bool> {
    *i += 1;
    while *i < buf.len() {
        let c = &mut buf[*i];
        *state = match state {
            Top => {
                *state = top(c, settings);
                if c.is_ascii_whitespace() {
                    *i += 1;
                    continue;
                }
                return Ok(*c == b'}' || *c == b']');
            }
            InString => in_string(*c),
            StringEscape => InString,
            InComment => in_comment(c, settings)?,
            InBlockComment => consume_block_comments(buf, i),
            MaybeCommentEnd => maybe_comment_end(c),
            InLineComment => consume_line_comments(buf, i),
        };
        *i += 1;
    }
    Ok(false)
}

fn strip_buf(state: &mut State, buf: &mut [u8], settings: CommentSettings) -> Result<()> {
    let mut i = 0;
    let len = buf.len();
    while i < len {
        let c = &mut buf[i];
        if matches!(state, Top) {
            let cur = i;
            *state = top(c, settings);
            if settings.trailing_commas
                && *c == b','
                && consume_comment_whitespace_until_maybe_bracket(state, buf, &mut i, settings)?
            {
                buf[cur] = b' ';
            }
        } else {
            *state = match state {
                Top => unreachable!(),
                InString => in_string(*c),
                StringEscape => InString,
                InComment => in_comment(c, settings)?,
                InBlockComment => consume_block_comments(buf, &mut i),
                MaybeCommentEnd => maybe_comment_end(c),
                InLineComment => consume_line_comments(buf, &mut i),
            }
        }
        i += 1;
    }
    Ok(())
}

#[inline]
fn consume_line_comments(buf: &mut [u8], i: &mut usize) -> State {
    let cur = *i;
    match memchr::memchr(b'\n', &buf[*i..]) {
        Some(offset) => {
            *i += offset;
            buf[cur..*i].fill(b' ');
            Top
        }
        None => {
            *i = buf.len() - 1;
            buf[cur..].fill(b' ');
            InLineComment
        }
    }
}

#[inline]
fn consume_block_comments(buf: &mut [u8], i: &mut usize) -> State {
    let cur = *i;
    match memchr::memchr(b'*', &buf[*i..]) {
        Some(offset) => {
            *i += offset;
            buf[cur..=*i].fill(b' ');
            MaybeCommentEnd
        }
        None => {
            *i = buf.len() - 1;
            buf[cur..].fill(b' ');
            InBlockComment
        }
    }
}

#[inline]
fn top(c: &mut u8, settings: CommentSettings) -> State {
    match *c {
        b'"' => InString,
        b'/' => {
            if settings.block_comments || settings.slash_line_comments {
                *c = b' ';
            }
            InComment
        }
        b'#' if settings.hash_line_comments => {
            *c = b' ';
            InLineComment
        }
        _ => Top,
    }
}

#[inline]
fn in_string(c: u8) -> State {
    match c {
        b'"' => Top,
        b'\\' => StringEscape,
        _ => InString,
    }
}

fn in_comment(c: &mut u8, settings: CommentSettings) -> Result<State> {
    let new_state = match c {
        b'*' if settings.block_comments => InBlockComment,
        b'/' if settings.slash_line_comments => InLineComment,
        _ => return Err(ErrorKind::InvalidData.into()),
    };
    *c = b' ';
    Ok(new_state)
}

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
    fn no_hash_comments() {
        let json = r#"# bad comment
        {"a": "b"}"#;
        let mut stripped = String::new();
        CommentSettings::c_style()
            .strip_comments(json.as_bytes())
            .read_to_string(&mut stripped)
            .unwrap();
        assert_eq!(stripped, json);
    }

    #[test]
    fn no_slash_line_comments() {
        let json = r#"// bad comment
        {"a": "b"}"#;
        let mut stripped = String::new();
        let err = CommentSettings::hash_only()
            .strip_comments(json.as_bytes())
            .read_to_string(&mut stripped)
            .unwrap_err();
        assert_eq!(err.kind(), ErrorKind::InvalidData);
    }

    #[test]
    fn no_block_comments() {
        let json = r#"/* bad comment */ {"a": "b"}"#;
        let mut stripped = String::new();
        let err = CommentSettings::hash_only()
            .strip_comments(json.as_bytes())
            .read_to_string(&mut stripped)
            .unwrap_err();
        assert_eq!(err.kind(), ErrorKind::InvalidData);
    }

    #[test]
    fn keep_all() {
        let original = String::from(
            r#"
     {
         "name": /* full */ "John Doe",
         "age": 43, # hash line comment
         "phones": [
             "+44 1234567", // work phone
             "+44 2345678", // home phone
         ], /** comment **/
     }"#,
        );
        let mut changed = original.clone();
        let _ = strip_comments_in_place(
            &mut changed,
            CommentSettings {
                block_comments: false,
                slash_line_comments: false,
                hash_line_comments: false,
                trailing_commas: false,
            },
        );
        assert_eq!(original, changed);
    }

    #[test]
    fn strip_in_place() {
        let mut json = String::from(r#"{/* Comment */"hi": /** abc */ "bye"}"#);
        strip_comments_in_place(&mut json, CommentSettings::default()).unwrap();
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
        strip_comments_in_place(&mut json, CommentSettings::default()).unwrap();

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
