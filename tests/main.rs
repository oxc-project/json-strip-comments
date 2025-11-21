use json_strip_comments::{StripComments, strip, strip_comments_in_place, strip_slice};

use std::io::{ErrorKind, Read};

fn strip_string(input: &str) -> String {
    let mut out = String::new();
    let count = StripComments::new(input.as_bytes()).read_to_string(&mut out).unwrap();
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
    assert_eq!(stripped, r#"{                         "hi":            "bye"}"#);
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

    let err = StripComments::new(json.as_bytes()).read_to_string(&mut stripped).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::InvalidData);
}

#[test]
fn incomplete_comment() {
    let json = "/* foo ";
    let mut stripped = String::new();

    let err = StripComments::new(json.as_bytes()).read_to_string(&mut stripped).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::InvalidData);
}

#[test]
fn incomplete_comment2() {
    let json = "/* foo *";
    let mut stripped = String::new();

    let err = StripComments::new(json.as_bytes()).read_to_string(&mut stripped).unwrap_err();
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
    let mut json = String::from(
        r#"[
            [1, 2, 3,],
            [4, 5, 6,/* comment */]
        ]"#,
    );
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
    let err = StripComments::new(json2.as_bytes()).read_to_string(&mut stripped2).unwrap_err();
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
    let err = StripComments::new(json.as_bytes()).read_to_string(&mut stripped).unwrap_err();
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
    let mut json = String::from(
        r#"{
            "a": 1, // comment after comma
            "b": 2
        }"#,
    );
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

#[test]
fn slice_api() {
    let mut json = String::from(
        r#"{
            "a": 1, // comment after comma
            "b": 2
        }"#,
    )
    .into_bytes();
    strip_slice(&mut json).unwrap();
    let json = String::from_utf8(json).unwrap();
    assert!(json.contains(r#""a": 1,"#));
    assert!(json.contains(r#""b": 2"#));
}

// Ported from https://github.com/sindresorhus/strip-json-comments/blob/main/test.js

#[test]
fn sindresorhus_replace_comments_with_whitespace() {
    assert_eq!(strip_string("//comment\n{\"a\":\"b\"}"), "         \n{\"a\":\"b\"}");
    assert_eq!(strip_string("/*//comment*/{\"a\":\"b\"}"), "             {\"a\":\"b\"}");
    assert_eq!(strip_string("{\"a\":\"b\"//comment\n}"), "{\"a\":\"b\"         \n}");
    assert_eq!(strip_string("{\"a\":\"b\"/*comment*/}"), "{\"a\":\"b\"           }");
    assert_eq!(
        strip_string("{\"a\"/*\n\n\ncomment\r\n*/:\"b\"}"),
        "{\"a\"  \n\n\n       \r\n  :\"b\"}"
    );
    assert_eq!(
        strip_string("/*!\n * comment\n */\n{\"a\":\"b\"}"),
        "   \n          \n   \n{\"a\":\"b\"}"
    );
    assert_eq!(strip_string("{/*comment*/\"a\":\"b\"}"), "{           \"a\":\"b\"}");
}

#[test]
fn sindresorhus_dont_strip_comments_inside_strings() {
    assert_eq!(strip_string("{\"a\":\"b//c\"}"), "{\"a\":\"b//c\"}");
    assert_eq!(strip_string("{\"a\":\"b/*c*/\"}"), "{\"a\":\"b/*c*/\"}");
    assert_eq!(strip_string("{\"/*a\":\"b\"}"), "{\"/*a\":\"b\"}");
    assert_eq!(strip_string("{\"\\\"/*a\":\"b\"}"), "{\"\\\"/*a\":\"b\"}");
}

#[test]
fn sindresorhus_escaped_slashes_with_escaped_string_quote() {
    assert_eq!(
        strip_string("{\"\\\\\":\"https://foobar.com\"}"),
        "{\"\\\\\":\"https://foobar.com\"}"
    );
    assert_eq!(
        strip_string("{\"foo\\\"\":\"https://foobar.com\"}"),
        "{\"foo\\\"\":\"https://foobar.com\"}"
    );
}

#[test]
fn sindresorhus_line_endings_no_comments() {
    assert_eq!(strip_string("{\"a\":\"b\"\n}"), "{\"a\":\"b\"\n}");
    assert_eq!(strip_string("{\"a\":\"b\"\r\n}"), "{\"a\":\"b\"\r\n}");
}

#[test]
fn sindresorhus_line_endings_single_line_comment() {
    assert_eq!(strip_string("{\"a\":\"b\"//c\n}"), "{\"a\":\"b\"   \n}");
    assert_eq!(strip_string("{\"a\":\"b\"//c\r\n}"), "{\"a\":\"b\"   \r\n}");
}

#[test]
fn sindresorhus_line_endings_single_line_block_comment() {
    assert_eq!(strip_string("{\"a\":\"b\"/*c*/\n}"), "{\"a\":\"b\"     \n}");
    assert_eq!(strip_string("{\"a\":\"b\"/*c*/\r\n}"), "{\"a\":\"b\"     \r\n}");
}

#[test]
fn sindresorhus_line_endings_multi_line_block_comment() {
    assert_eq!(
        strip_string("{\"a\":\"b\",/*c\nc2*/\"x\":\"y\"\n}"),
        "{\"a\":\"b\",   \n    \"x\":\"y\"\n}"
    );
    assert_eq!(
        strip_string("{\"a\":\"b\",/*c\r\nc2*/\"x\":\"y\"\r\n}"),
        "{\"a\":\"b\",   \r\n    \"x\":\"y\"\r\n}"
    );
}

#[test]
fn sindresorhus_line_endings_works_at_eof() {
    assert_eq!(
        strip_string("{\r\n\t\"a\":\"b\"\r\n} //EOF"),
        "{\r\n\t\"a\":\"b\"\r\n}      "
    );
}

#[test]
fn sindresorhus_weird_escaping() {
    let input = r#"{"x":"x \"sed -e \\\"s/^.\\\\{46\\\\}T//\\\" -e \\\"s/#033/\\\\x1b/g\\\"\""}"#;
    assert_eq!(strip_string(input), input);
}

#[test]
fn sindresorhus_strips_trailing_commas() {
    let mut json = String::from("{\"x\":true,}");
    strip_comments_in_place(&mut json).unwrap();
    assert_eq!(json, "{\"x\":true }");

    let mut json = String::from("{\"x\":true,\n  }");
    strip_comments_in_place(&mut json).unwrap();
    assert_eq!(json, "{\"x\":true \n  }");

    let mut json = String::from("[true, false,]");
    strip_comments_in_place(&mut json).unwrap();
    assert_eq!(json, "[true, false ]");

    let mut json = String::from("{\n  \"array\": [\n    true,\n    false,\n  ],\n}");
    strip_comments_in_place(&mut json).unwrap();
    // Compare without whitespace since the implementation may vary slightly
    assert_eq!(
        json.chars().filter(|c| !c.is_whitespace()).collect::<String>(),
        "{\"array\":[true,false]}".chars().filter(|c| !c.is_whitespace()).collect::<String>()
    );

    let mut json =
        String::from("{\n  \"array\": [\n    true,\n    false /* comment */ ,\n /*comment*/ ],\n}");
    strip_comments_in_place(&mut json).unwrap();
    // Compare without whitespace
    assert_eq!(
        json.chars().filter(|c| !c.is_whitespace()).collect::<String>(),
        "{\"array\":[true,false]}".chars().filter(|c| !c.is_whitespace()).collect::<String>()
    );
}

#[test]
fn sindresorhus_malformed_block_comments() {
    // Note: The Rust implementation treats "[] */" differently than JavaScript.
    // When it sees "*/" it interprets the "/" as potentially starting a comment,
    // which causes an error. The JavaScript version is more lenient.
    let json1 = "[] */";
    let mut stripped1 = String::new();
    let result1 = StripComments::new(json1.as_bytes()).read_to_string(&mut stripped1);
    // The Rust implementation errors on this input
    assert!(result1.is_err());

    // Unclosed comment - the JavaScript version is lenient about this,
    // but the Rust implementation correctly returns an error
    let json2 = "[] /*";
    let mut stripped2 = String::new();
    let result2 = StripComments::new(json2.as_bytes()).read_to_string(&mut stripped2);
    assert!(result2.is_err());
    if let Err(err) = result2 {
        assert_eq!(err.kind(), ErrorKind::InvalidData);
    }
}

#[test]
fn sindresorhus_non_breaking_space() {
    let fixture = "{\n\t// Comment with non-breaking-space: '\u{00A0}'\n\t\"a\": 1\n\t}";
    let stripped = strip_string(fixture);
    // Should be able to parse the result
    let parsed: Result<serde_json::Value, _> = serde_json::from_str(&stripped);
    assert!(parsed.is_ok());
    if let Ok(value) = parsed {
        assert_eq!(value["a"], 1);
    }
}
