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
    Top = 0,           // Most common state - make it 0 for better branch prediction
    InString = 1,      // Second most common
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


#[inline]
fn consume_comment_whitespace_until_maybe_bracket(
    state: &mut State,
    buf: &mut [u8],
    i: &mut usize,
) -> Result<bool> {
    *i += 1;
    let len = buf.len();
    while *i < len {
        let c = &mut buf[*i];
        *state = match *state {
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

    while i < len {
        let c = &mut buf[i];

        match *state {
            Top => {
                let byte = *c;  // Cache byte before top() modifies it
                let cur = i;
                let new_state = top(c);
                if byte == b',' {
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
#[cold]
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

    #[test]
    fn nested_block_comments() {
        let json = r#"{/* /* nested */ "foo": 123 }"#;
        let stripped = strip_string(json);
        assert_eq!(stripped, r#"{                "foo": 123 }"#);
    }

    #[test]
    fn mixed_comment_types() {
        let json = r#"{
            // line before block
            /* block comment */
            "a": 1,
            # shell comment before line
            // c line comment
            "b": 2
        }"#;
        let stripped = strip_string(json);
        assert!(stripped.contains(r#""a": 1"#));
        assert!(stripped.contains(r#""b": 2"#));
    }

    #[test]
    fn comment_like_strings() {
        let json = r##"{
            "url": "http://example.com",
            "comment": "// this is not a comment",
            "block": "/* neither is this */",
            "shell": "# nor this"
        }"##;
        let stripped = strip_string(json);
        assert!(stripped.contains("// this is not a comment"));
        assert!(stripped.contains("/* neither is this */"));
        assert!(stripped.contains(r#"# nor this"#));
    }

    #[test]
    fn escaped_quotes_in_strings() {
        let json = r#"{
            "escaped": "He said \"hello\" to me",
            /* comment */
            "normal": "value"
        }"#;
        let stripped = strip_string(json);
        assert!(stripped.contains(r#"\"hello\""#));
    }

    #[test]
    fn empty_json() {
        let json = "";
        let stripped = strip_string(json);
        assert_eq!(stripped, "");
    }

    #[test]
    fn only_comments() {
        let json = "/* just a comment */";
        let stripped = strip_string(json);
        assert_eq!(stripped, "                    ");

        let json2 = "// only line comment";
        let stripped2 = strip_string(json2);
        assert_eq!(stripped2, "                    ");
    }

    #[test]
    fn trailing_comma_nested_arrays() {
        let mut json = String::from(r#"[
            [1, 2, 3,],
            [4, 5, 6,/* comment */]
        ]"#);
        strip_comments_in_place(&mut json).unwrap();

        // The comment is 13 characters: /* comment */
        // So it should be replaced with 13 spaces
        assert!(json.contains("[1, 2, 3 ]"));
        assert!(json.contains("[4, 5, 6              ]"));
    }

    #[test]
    fn multiline_strings_with_comments() {
        let json = r#"{
            "multiline": "line1
line2", // comment after multiline string
            "next": "value"
        }"#;
        let stripped = strip_string(json);
        assert!(stripped.contains("line1\nline2"));
        assert!(stripped.contains(r#""next": "value""#));
    }

    #[test]
    fn unicode_in_strings() {
        let json = r#"{
            "unicode": "Hello 世界 🌍", // comment with unicode too: 你好
            "emoji": "🎉🎊"
        }"#;
        let stripped = strip_string(json);
        assert!(stripped.contains("Hello 世界 🌍"));
        assert!(stripped.contains("🎉🎊"));
    }

    #[test]
    fn comment_at_eof() {
        let json = r#"{"a": 1} // comment at end"#;
        let stripped = strip_string(json);
        assert_eq!(stripped, r#"{"a": 1}                  "#);

        let json2 = r#"{"a": 1} /* block at end"#;
        let mut stripped2 = String::new();
        let err = StripComments::new(json2.as_bytes())
            .read_to_string(&mut stripped2)
            .unwrap_err();
        assert_eq!(err.kind(), ErrorKind::InvalidData);
    }

    #[test]
    fn consecutive_comments() {
        let json = r#"{
            // comment 1
            // comment 2
            /* block 1 *//* block 2 */
            "key": "value"
        }"#;
        let stripped = strip_string(json);
        assert!(stripped.contains(r#""key": "value""#));
    }

    #[test]
    fn backslash_before_quote() {
        let json = r#"{
            "path": "C:\\", // comment
            "escaped_backslash": "\\\""
        }"#;
        let stripped = strip_string(json);
        assert!(stripped.contains("C:\\\\"));
        assert!(stripped.contains("\\\\\\\""));
    }

    #[test]
    fn invalid_comment_start() {
        let json = "/not-a-comment";
        let mut stripped = String::new();
        let err = StripComments::new(json.as_bytes())
            .read_to_string(&mut stripped)
            .unwrap_err();
        assert_eq!(err.kind(), ErrorKind::InvalidData);
    }

    #[test]
    fn long_block_comment() {
        let comment_content = "x".repeat(10000);
        let json = format!(r#"{{/* {} */"key": "value"}}"#, comment_content);
        let stripped = strip_string(&json);
        assert!(stripped.contains(r#""key": "value""#));
        assert_eq!(stripped.len(), json.len());
    }

    #[test]
    fn whitespace_only() {
        let json = "   \n\t  \r\n  ";
        let stripped = strip_string(json);
        assert_eq!(stripped, json);
    }

    #[test]
    fn comment_before_colon() {
        let json = r#"{
            "key"/* comment */: "value"
        }"#;
        let stripped = strip_string(json);
        assert!(stripped.contains(r#""key"             : "value""#));
    }

    #[test]
    fn comment_in_array() {
        let json = r#"[
            1, // first
            2, /* second */
            3  # third
        ]"#;
        let stripped = strip_string(json);
        assert!(stripped.contains("1,"));
        assert!(stripped.contains("2,"));
        assert!(stripped.contains("3"));
    }

    #[test]
    fn partial_read() {
        let json = r#"{/* Comment that spans multiple reads */ "key": "value"}"#;
        let mut reader = StripComments::new(json.as_bytes());
        let mut buf = [0u8; 10];
        let mut result = Vec::new();

        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => result.extend_from_slice(&buf[..n]),
                Err(_) => panic!("Unexpected error"),
            }
        }

        let stripped = String::from_utf8(result).unwrap();
        assert!(stripped.contains(r#""key": "value""#));
    }

    #[test]
    fn read_exact_behavior() {
        use std::io::Read;

        let json = r#"{"a": 1, /* comment */ "b": 2}"#;
        let mut reader = StripComments::new(json.as_bytes());
        let mut buf = [0u8; 5];

        reader.read_exact(&mut buf).unwrap();
        assert_eq!(&buf, b"{\"a\":");
    }

    #[test]
    fn zero_sized_read() {
        let json = r#"{"key": "value"}"#;
        let mut reader = StripComments::new(json.as_bytes());
        let mut buf = [];

        let n = reader.read(&mut buf).unwrap();
        assert_eq!(n, 0);
    }

    #[test]
    fn strip_alias_function() {
        let mut json = String::from(r#"{/* test */ "a": 1}"#);
        strip(&mut json).unwrap();
        assert_eq!(json, r#"{           "a": 1}"#);
    }

    #[test]
    fn invalid_escape_sequence() {
        let json = r#"{"key": "value\x"}"#;
        let stripped = strip_string(json);
        assert_eq!(stripped, json);
    }

    #[test]
    fn multiple_string_escapes() {
        let json = r#"{"escaped": "\\\"\n\r\t", /* comment */ "next": 1}"#;
        let stripped = strip_string(json);
        assert!(stripped.contains("\\\\\\\"\\n\\r\\t"));
    }

    #[test]
    fn large_input_streaming() {
        let mut large_json = String::from("{");
        for i in 0..1000 {
            large_json.push_str(&format!(r#""key{}": {} /* comment {} */,"#, i, i, i));
        }
        large_json.push_str(r#""final": 1000}"#);

        let stripped = strip_string(&large_json);
        assert!(stripped.contains(r#""key999": 999"#));
        assert!(stripped.contains(r#""final": 1000"#));
    }

    #[test]
    fn comment_after_comma_in_object() {
        let mut json = String::from(r#"{
            "a": 1, // comment after comma
            "b": 2
        }"#);
        strip_comments_in_place(&mut json).unwrap();
        assert!(json.contains(r#""a": 1,"#));
        assert!(json.contains(r#""b": 2"#));
    }

    #[test]
    fn special_characters_in_comments() {
        let json = r#"{
            /* Special chars: @#$%^&*() */
            "key": "value",
            // More special: <>?[]{}|~`
            "key2": "value2"
        }"#;
        let stripped = strip_string(json);
        assert!(stripped.contains(r#""key": "value""#));
        assert!(stripped.contains(r#""key2": "value2""#));
    }
}
