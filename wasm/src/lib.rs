use serde::Deserialize;
use tsify::Tsify;
use wasm_bindgen::prelude::*;

/// Legacy CommentSettings for backward compatibility.
/// Note: These settings are now ignored as the library always strips all comment types.
#[derive(Debug, Default, Clone, Deserialize, Tsify)]
#[tsify(from_wasm_abi)]
#[serde(rename_all = "camelCase")]
pub struct CommentSettings {
    /// True if c-style block comments (`/* ... */`) are removed.
    /// @deprecated This setting is ignored. All comment types are always removed.
    /// @default true
    #[tsify(optional)]
    pub block_comments: Option<bool>,

    /// True if c-style `//` line comments are removed.
    /// @deprecated This setting is ignored. All comment types are always removed.
    /// @default true
    #[tsify(optional)]
    pub slash_line_comments: Option<bool>,

    /// True if shell-style `#` line comments are removed.
    /// @deprecated This setting is ignored. All comment types are always removed.
    /// @default true
    #[tsify(optional)]
    pub hash_line_comments: Option<bool>,

    /// True if trailing commas are removed.
    /// @deprecated This setting is ignored. Trailing commas are always removed.
    /// @default true
    #[tsify(optional)]
    pub trailing_commas: Option<bool>,
}

/// Strips comments and trailing commas by replacing them with whitespaces.
/// Note: The settings parameter is kept for backward compatibility but is ignored.
/// All comment types (block, line, hash) and trailing commas are always removed.
#[wasm_bindgen]
pub fn strip(string: String, _settings: Option<CommentSettings>) -> String {
    let mut string = string;
    let _ = json_strip_comments::strip_comments_in_place(&mut string);
    string
}
