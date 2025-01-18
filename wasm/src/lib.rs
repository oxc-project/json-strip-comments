// use json_strip_comments::{strip, CommentSettings};
use serde::Deserialize;
use tsify::Tsify;
use wasm_bindgen::prelude::*;

#[derive(Debug, Default, Clone, Deserialize, Tsify)]
#[tsify(from_wasm_abi)]
#[serde(rename_all = "camelCase")]
pub struct CommentSettings {
    /// True if c-style block comments (`/* ... */`) are removed.
    ///
    /// @default true
    #[tsify(optional)]
    pub block_comments: Option<bool>,

    /// True if c-style `//` line comments are removed.
    ///
    /// @default true
    #[tsify(optional)]
    pub slash_line_comments: Option<bool>,

    /// True if shell-style `#` line comments are removed.
    ///
    /// @default true
    #[tsify(optional)]
    pub hash_line_comments: Option<bool>,

    /// True if trailing commas are removed.
    ///
    /// @default true
    #[tsify(optional)]
    pub trailing_commas: Option<bool>,
}

/// Strips comments and trailing commas by replacing them with whitespaces.
#[wasm_bindgen]
pub fn strip(string: String, settings: Option<CommentSettings>) -> String {
    let mut string = string;
    let settings = settings.unwrap_or_default();
    let remove_trailing_commas = settings.trailing_commas.unwrap_or(true);
    let settings = json_strip_comments::CommentSettings {
        block_comments: settings.block_comments.unwrap_or(true),
        slash_line_comments: settings.slash_line_comments.unwrap_or(true),
        hash_line_comments: settings.hash_line_comments.unwrap_or(true),
    };
    let _ =
        json_strip_comments::strip_comments_in_place(&mut string, settings, remove_trailing_commas);
    string
}
