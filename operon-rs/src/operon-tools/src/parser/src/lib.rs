//! `operon-tools-parser` Crate
//!
//! Hello friend! This crate implements a custom, robust, and deterministic parser
//! designed to parse tool calls emitted by an LLM (Large Language Model) in a custom
//! XML-like format. The LLM can emit tool calls interleaved with natural language prose.
//!
//! ### Tag Formats
//! The model supports two types of tool calls:
//! 1. **Bodyless tool calls**: Single-line calls containing only attributes, e.g.:
//!    `<read paths="C:\src\main.rs" "C:\src\lib.rs:10-50">`
//! 2. **Body tool calls**: Multiline calls with custom delimiter blocks `<<<<` and `>>>>`, e.g.:
//!    `<write path="C:\src\new.rs">`
//!    `<<<<`
//!    `fn main() {`
//!    `    println!("hello");`
//!    `}`
//!    `>>>>`
//!
//! ### How the Parser Works
//! The parser reads the entire LLM response sequentially. It uses a line-based lookahead
//! approach after finding a closing tag `>` to determine if the call is followed by a body
//! delimiter (`<<<<`). It then separates all the valid tool calls, reports errors for
//! malformed ones, and constructs a "cleaned" prose text where all tool-call tags and
//! body delimiter/content blocks are stripped out, collapsing consecutive empty lines.

use std::collections::HashMap;

/// A raw parsed tool call containing structural information about the call.
/// This will be converted to a normal dispatchable tool call later in the pipeline.
#[derive(Debug, Clone, PartialEq)]
pub struct RawToolCall {
    /// The name of the tool, extracted from the tag (e.g. "read", "write", "bash").
    pub name: String,

    /// Flat string-to-string attribute map.
    /// If the tool has a body, it is stored under the special key `"__body__"`.
    /// All values are unquoted and backslash-unescaped.
    pub attrs: HashMap<String, String>,

    /// The zero-based byte offset of the opening `<` character in the original text.
    /// This is useful for precise log attribution and error mapping.
    pub offset: usize,
}

/// The result of parsing a complete model response.
#[derive(Debug, Clone, PartialEq)]
pub struct ParseResult {
    /// The successfully parsed tool calls, in the order they were emitted.
    pub calls: Vec<RawToolCall>,

    /// The model's response text with all tool-call tags and body blocks stripped out.
    /// Consecutive blank lines left by stripping are collapsed into a single blank line.
    pub text: String,
}

impl RawToolCall {
    /// Converts this `RawToolCall` into a dispatchable `ToolCall` from the
    /// `operon-context-normalize-tools` crate.
    ///
    /// This assigns a unique `ToolCallId` and maps the attribute strings into a
    /// `serde_json::Value::Object`.
    pub fn into_tool_call(
        self,
        call_id: operon_context_normalize_tools::ToolCallId,
    ) -> operon_context_normalize_tools::ToolCall {
        // We pack all flat string attributes into a serde JSON object where values are string types.
        let arguments = serde_json::Value::Object(
            self.attrs
                .into_iter()
                .map(|(k, v)| (k, serde_json::Value::String(v)))
                .collect(),
        );

        operon_context_normalize_tools::ToolCall {
            id: call_id,
            name: self.name,
            arguments,
        }
    }
}

/// A structure representing a parsing error, along with the byte offset of the tag.
#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    /// The zero-based byte offset of the opening `<` for the tag where the error occurred.
    pub offset: usize,
    /// The type/kind of error encountered.
    pub kind: ParseErrorKind,
}

/// Represents the different ways a tool-call tag block can be malformed.
#[derive(Debug, Clone, PartialEq)]
pub enum ParseErrorKind {
    /// An attribute value was not enclosed in double quotes (e.g. `path=C:\src\main.rs`).
    UnquotedAttrValue { key: String },
    /// The same attribute key was specified multiple times explicitly.
    DuplicateAttr { key: String },
    /// The opening `<` was found, but the tag line was never closed with a `>`.
    UnclosedTag,
    /// The `<<<<` body delimiter was found, but the closing `>>>>` was never encountered.
    UnclosedBody,
    /// The tool name was empty or contained invalid characters (not matching `[a-zA-Z][a-zA-Z0-9_]*`).
    InvalidToolName,
}

/// Parses a complete model response text and returns parsed tool calls + cleaned prose.
/// This function never panics. Malformed tag blocks are silently skipped in the
/// output tool calls list, allowing the model to see no results and self-correct.
pub fn parse(text: &str) -> ParseResult {
    let (result, _) = parse_with_errors(text);
    result
}

/// Parses a complete model response text, returning both the `ParseResult` and a list of
/// diagnostic `ParseError`s representing any malformed tags.
pub fn parse_with_errors(text: &str) -> (ParseResult, Vec<ParseError>) {
    let mut calls = Vec::new();
    let mut errors = Vec::new();
    let mut strip_ranges = Vec::new();

    let mut pos = 0;
    let len = text.len();

    while pos < len {
        // 1. Search for the next potential tag starting character '<' followed by an alphabetic char.
        let start_pos = match find_next_tag_start(text, pos) {
            Some(idx) => idx,
            None => break,
        };

        // 2. Find the closing '>' of this tag.
        let tag_end_pos = match find_tag_end(text, start_pos) {
            Some(idx) => idx,
            None => {
                // If the tag is never closed, we record an UnclosedTag error.
                errors.push(ParseError {
                    offset: start_pos,
                    kind: ParseErrorKind::UnclosedTag,
                });
                // We strip the unclosed tag block from the start `<` to the end of the text.
                strip_ranges.push((start_pos, len));
                break;
            }
        };

        // 3. Extract the tag content (excluding the '<' and '>') and split tool name from attributes.
        let tag_str = &text[start_pos + 1..tag_end_pos];
        let first_ws = tag_str.find(|c: char| c.is_whitespace());
        let (tool_name, attr_str) = match first_ws {
            Some(idx) => {
                let name = &tag_str[..idx];
                let attrs = &tag_str[idx..];
                (name, attrs)
            }
            None => (tag_str, ""),
        };

        // 4. Validate the tool name format.
        if !is_valid_tool_name(tool_name) {
            errors.push(ParseError {
                offset: start_pos,
                kind: ParseErrorKind::InvalidToolName,
            });
            // Strip the malformed tag block and move pos forward.
            strip_ranges.push((start_pos, tag_end_pos + 1));
            pos = tag_end_pos + 1;
            continue;
        }

        // 5. Parse the attribute string.
        let attrs = match parse_attributes(attr_str) {
            Ok(map) => map,
            Err(kind) => {
                errors.push(ParseError {
                    offset: start_pos,
                    kind,
                });
                // Strip the malformed tag block and move pos forward.
                strip_ranges.push((start_pos, tag_end_pos + 1));
                pos = tag_end_pos + 1;
                continue;
            }
        };

        // 6. Lookahead to check if the next non-empty line starts with the body delimiter "<<<<".
        let nl_pos = match text[tag_end_pos..].find('\n') {
            Some(offset) => tag_end_pos + offset,
            None => len,
        };

        let mut lookahead_pos = if nl_pos < len { nl_pos + 1 } else { len };
        let mut found_body_delimiter = false;
        let mut bodyless_resume_pos = lookahead_pos;
        let mut delimiter_line_start = 0;
        let mut delimiter_line_end = 0;

        while lookahead_pos < len {
            let line_end = match text[lookahead_pos..].find('\n') {
                Some(offset) => lookahead_pos + offset,
                None => len,
            };
            let line_content = &text[lookahead_pos..line_end];
            let trimmed = line_content.trim();

            if trimmed.is_empty() {
                // Skip empty lines when looking for body delimiters.
                lookahead_pos = if line_end < len { line_end + 1 } else { len };
            } else if trimmed == "<<<<" {
                found_body_delimiter = true;
                delimiter_line_start = lookahead_pos;
                delimiter_line_end = if line_end < len { line_end + 1 } else { len };
                break;
            } else {
                // Found some prose or a new tag.
                bodyless_resume_pos = lookahead_pos;
                break;
            }
        }

        if found_body_delimiter {
            // 7. We found a body delimiter! Now scan for the closing delimiter ">>>>".
            let mut body_pos = delimiter_line_end;
            let mut found_closing_delimiter = false;
            let mut closing_line_start = 0;
            let mut closing_line_end = 0;

            while body_pos < len {
                let line_end = match text[body_pos..].find('\n') {
                    Some(offset) => body_pos + offset,
                    None => len,
                };
                let line_content = &text[body_pos..line_end];
                let trimmed = line_content.trim();

                if trimmed == ">>>>" {
                    found_closing_delimiter = true;
                    closing_line_start = body_pos;
                    closing_line_end = if line_end < len { line_end + 1 } else { len };
                    break;
                }

                body_pos = if line_end < len { line_end + 1 } else { len };
            }

            if !found_closing_delimiter {
                errors.push(ParseError {
                    offset: start_pos,
                    kind: ParseErrorKind::UnclosedBody,
                });
                // Strip the entire unclosed body range to the end of the text.
                strip_ranges.push((start_pos, len));
                pos = len;
                continue;
            }

            // Extract the raw body content.
            let mut raw_body = &text[delimiter_line_start + 4..closing_line_start];

            // Strip exactly one leading newline (CRLF or LF).
            if raw_body.starts_with('\n') {
                raw_body = &raw_body[1..];
            } else if raw_body.starts_with("\r\n") {
                raw_body = &raw_body[2..];
            }

            // Strip exactly one trailing newline (CRLF or LF).
            if raw_body.ends_with('\n') {
                raw_body = &raw_body[..raw_body.len() - 1];
                if raw_body.ends_with('\r') {
                    raw_body = &raw_body[..raw_body.len() - 1];
                }
            }

            let mut final_attrs = attrs;
            final_attrs.insert("__body__".to_string(), raw_body.to_string());

            calls.push(RawToolCall {
                name: tool_name.to_string(),
                attrs: final_attrs,
                offset: start_pos,
            });

            // Strip the complete tag + body block.
            strip_ranges.push((start_pos, closing_line_end));
            pos = closing_line_end;
        } else {
            // 8. Bodyless tool call.
            calls.push(RawToolCall {
                name: tool_name.to_string(),
                attrs,
                offset: start_pos,
            });

            // Strip the single tag line.
            strip_ranges.push((start_pos, tag_end_pos + 1));
            pos = bodyless_resume_pos;
        }
    }

    // 9. Reconstruct the cleaned prose by skipping all the stripped tag ranges.
    let mut cleaned_parts = Vec::new();
    let mut current_idx = 0;
    for &(start, end) in &strip_ranges {
        if start > current_idx {
            cleaned_parts.push(&text[current_idx..start]);
        }
        current_idx = end;
    }
    if current_idx < len {
        cleaned_parts.push(&text[current_idx..len]);
    }

    let joined_cleaned = cleaned_parts.concat();

    // 10. Split the cleaned text into lines, strip CR, and collapse consecutive blank lines.
    let lines = split_lines(&joined_cleaned);
    let collapsed_text = collapse_whitespace_lines(&lines);

    (
        ParseResult {
            calls,
            text: collapsed_text,
        },
        errors,
    )
}

/// Scans the text forward from `start_pos` looking for `<` followed immediately
/// by an ASCII alphabetic letter (indicating the start of a tag).
fn find_next_tag_start(text: &str, start_pos: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut i = start_pos;
    while i < len {
        if bytes[i] == b'<' {
            if i + 1 < len {
                let next_char = bytes[i + 1] as char;
                if next_char.is_ascii_alphabetic() {
                    return Some(i);
                }
            }
        }
        i += 1;
    }
    None
}

/// Scans forward from `start_pos` to find the first occurrence of a closing `>`.
fn find_tag_end(text: &str, start_pos: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut i = start_pos;
    while i < len {
        if bytes[i] == b'>' {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// Validates if a tool name matches the regex pattern `^[a-zA-Z][a-zA-Z0-9_]*$`.
fn is_valid_tool_name(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    let bytes = name.as_bytes();
    if !bytes[0].is_ascii_alphabetic() {
        return false;
    }
    for &b in &bytes[1..] {
        if !(b.is_ascii_alphanumeric() || b == b'_') {
            return false;
        }
    }
    true
}

/// Parses the attribute string between the tool name and the closing `>`.
///
/// Supports:
/// - Key-value tokens: `key="value"`
/// - Keyless quoted values (positional args): `"value"`, which are appended to the `"paths"` key.
/// - Unescaping values: `\"` becomes `"`, `\\` becomes `\`. All other escapes are left as-is.
/// - Duplicate explicit key detection.
/// - Ignored keys with invalid characters (ASCII alphanumeric and underscore only).
fn parse_attributes(attr_str: &str) -> Result<HashMap<String, String>, ParseErrorKind> {
    let mut attrs = HashMap::new();
    let mut explicit_keys = std::collections::HashSet::new();
    let chars: Vec<char> = attr_str.chars().collect();
    let mut pos = 0;
    let len = chars.len();

    while pos < len {
        // Skip whitespace.
        while pos < len && chars[pos].is_whitespace() {
            pos += 1;
        }
        if pos == len {
            break;
        }

        // Handle keyless quoted values (e.g. `"C:\src\lib.rs:10-50"`).
        if chars[pos] == '"' {
            pos += 1; // Consume the opening quote.
            let mut value_chars = Vec::new();
            let mut closed_quote = false;

            while pos < len {
                let c = chars[pos];
                if c == '\\' {
                    if pos + 1 < len {
                        let next_c = chars[pos + 1];
                        if next_c == '"' {
                            value_chars.push('"');
                            pos += 2;
                        } else if next_c == '\\' {
                            value_chars.push('\\');
                            pos += 2;
                        } else {
                            value_chars.push('\\');
                            pos += 1;
                        }
                    } else {
                        value_chars.push('\\');
                        pos += 1;
                    }
                } else if c == '"' {
                    closed_quote = true;
                    pos += 1; // Consume closing quote.
                    break;
                } else {
                    value_chars.push(c);
                    pos += 1;
                }
            }

            if !closed_quote {
                return Err(ParseErrorKind::UnquotedAttrValue {
                    key: "".to_string(),
                });
            }

            let value: String = value_chars.into_iter().collect();
            // Keyless values are appended to the "paths" key separated by a space.
            let entry = attrs.entry("paths".to_string()).or_insert_with(String::new);
            if !entry.is_empty() {
                entry.push(' ');
            }
            entry.push_str(&value);
            continue;
        }

        // Handle key-value pairs (e.g. `path="C:\src\new.rs"`).
        let mut key_chars = Vec::new();
        let mut has_equals = false;
        let mut temp_pos = pos;
        let mut has_invalid_key_char = false;

        while temp_pos < len && !chars[temp_pos].is_whitespace() {
            if chars[temp_pos] == '=' {
                has_equals = true;
                break;
            }
            let c = chars[temp_pos];
            if !(c.is_ascii_alphanumeric() || c == '_') {
                has_invalid_key_char = true;
            }
            key_chars.push(c);
            temp_pos += 1;
        }

        if !has_equals {
            // If there's no '=' and it's not a quoted value, it's an unquoted/malformed attribute.
            let word: String = key_chars.into_iter().collect();
            return Err(ParseErrorKind::UnquotedAttrValue { key: word });
        }

        pos = temp_pos + 1; // Move past the '=' character.

        // Skip whitespace after '='.
        while pos < len && chars[pos].is_whitespace() {
            pos += 1;
        }

        // Value must be double-quoted.
        if pos == len || chars[pos] != '"' {
            let key: String = key_chars.into_iter().collect();
            return Err(ParseErrorKind::UnquotedAttrValue { key });
        }

        pos += 1; // Consume the opening quote.

        let mut value_chars = Vec::new();
        let mut closed_quote = false;

        while pos < len {
            let c = chars[pos];
            if c == '\\' {
                if pos + 1 < len {
                    let next_c = chars[pos + 1];
                    if next_c == '"' {
                        value_chars.push('"');
                        pos += 2;
                    } else if next_c == '\\' {
                        value_chars.push('\\');
                        pos += 2;
                    } else {
                        value_chars.push('\\');
                        pos += 1;
                    }
                } else {
                    value_chars.push('\\');
                    pos += 1;
                }
            } else if c == '"' {
                closed_quote = true;
                pos += 1; // Consume closing quote.
                break;
            } else {
                value_chars.push(c);
                pos += 1;
            }
        }

        if !closed_quote {
            let key: String = key_chars.into_iter().collect();
            return Err(ParseErrorKind::UnquotedAttrValue { key });
        }

        let key: String = key_chars.into_iter().collect();
        let value: String = value_chars.into_iter().collect();

        // If the key has invalid characters, we silently ignore the attribute (forward compatibility).
        if !has_invalid_key_char {
            // Reject duplicate explicit keys.
            if explicit_keys.contains(&key) {
                return Err(ParseErrorKind::DuplicateAttr { key });
            }
            explicit_keys.insert(key.clone());

            let entry = attrs.entry(key).or_insert_with(String::new);
            if !entry.is_empty() {
                entry.push(' ');
            }
            entry.push_str(&value);
        }
    }

    Ok(attrs)
}

/// Splits a string by newline (`\n`) and strips any trailing carriage return (`\r`).
fn split_lines(text: &str) -> Vec<&str> {
    text.split('\n')
        .map(|s| s.strip_suffix('\r').unwrap_or(s))
        .collect()
}

/// Collapses consecutive whitespace-only or empty lines down to a single blank line.
fn collapse_whitespace_lines(lines: &[&str]) -> String {
    let mut result = Vec::new();
    let mut was_empty = false;
    for &line in lines {
        let is_empty = line.trim().is_empty();
        if is_empty {
            if !was_empty {
                result.push("");
                was_empty = true;
            }
        } else {
            result.push(line);
            was_empty = false;
        }
    }
    result.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bodyless_single_call() {
        let text = "<read paths=\"C:\\src\\main.rs\">";
        let result = parse(text);
        assert_eq!(result.calls.len(), 1);
        assert_eq!(result.calls[0].name, "read");
        assert_eq!(result.calls[0].attrs.get("paths").unwrap(), "C:\\src\\main.rs");
        assert_eq!(result.text, "");
    }

    #[test]
    fn test_body_call() {
        let text = "<write path=\"C:\\src\\new.rs\">\n<<<<\nfn main() {\n    println!(\"hello\");\n}\n>>>>";
        let result = parse(text);
        assert_eq!(result.calls.len(), 1);
        assert_eq!(result.calls[0].name, "write");
        assert_eq!(result.calls[0].attrs.get("path").unwrap(), "C:\\src\\new.rs");
        assert_eq!(
            result.calls[0].attrs.get("__body__").unwrap(),
            "fn main() {\n    println!(\"hello\");\n}"
        );
        assert_eq!(result.text, "");
    }

    #[test]
    fn test_multiple_calls_mixed_with_prose() {
        let text = "I'll read both files first.\n\n<read paths=\"C:\\src\\main.rs\" \"C:\\src\\lib.rs:10-50\">\n\nNow I'll write the new file.\n\n<write path=\"C:\\src\\new.rs\">\n<<<<\nfn main() {\n    println!(\"hello\");\n}\n>>>>\n\nDone.";
        let result = parse(text);
        assert_eq!(result.calls.len(), 2);
        assert_eq!(result.calls[0].name, "read");
        assert_eq!(
            result.calls[0].attrs.get("paths").unwrap(),
            "C:\\src\\main.rs C:\\src\\lib.rs:10-50"
        );
        assert_eq!(result.calls[1].name, "write");
        assert_eq!(result.calls[1].attrs.get("path").unwrap(), "C:\\src\\new.rs");
        assert_eq!(
            result.calls[1].attrs.get("__body__").unwrap(),
            "fn main() {\n    println!(\"hello\");\n}"
        );
        assert_eq!(
            result.text,
            "I'll read both files first.\n\nNow I'll write the new file.\n\nDone."
        );
    }

    #[test]
    fn test_attr_with_windows_path_escaping() {
        let text = "<read paths=\"C:\\foo\\bar.txt\" extra=\"some\\\"escaped\\\"value\\\\here\">";
        let result = parse(text);
        assert_eq!(result.calls.len(), 1);
        assert_eq!(result.calls[0].attrs.get("paths").unwrap(), "C:\\foo\\bar.txt");
        assert_eq!(result.calls[0].attrs.get("extra").unwrap(), "some\"escaped\"value\\here");
    }

    #[test]
    fn test_unquoted_attr_value() {
        let text = "<read paths=C:\\src\\main.rs>";
        let (result, errors) = parse_with_errors(text);
        assert!(result.calls.is_empty());
        assert_eq!(errors.len(), 1);
        match &errors[0].kind {
            ParseErrorKind::UnquotedAttrValue { key } => {
                assert_eq!(key, "paths");
            }
            _ => panic!("Expected UnquotedAttrValue"),
        }
    }

    #[test]
    fn test_unclosed_body() {
        let text = "<write path=\"C:\\src\\new.rs\">\n<<<<\nfn main() {}";
        let (result, errors) = parse_with_errors(text);
        assert!(result.calls.is_empty());
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].kind, ParseErrorKind::UnclosedBody);
    }

    #[test]
    fn test_prose_only_response() {
        let text = "This is a simple text response with no tool calls.";
        let result = parse(text);
        assert!(result.calls.is_empty());
        assert_eq!(result.text, text);
    }

    #[test]
    fn test_empty_body() {
        let text = "<write path=\"C:\\src\\new.rs\">\n<<<<\n>>>>";
        let result = parse(text);
        assert_eq!(result.calls.len(), 1);
        assert_eq!(result.calls[0].attrs.get("__body__").unwrap(), "");
    }

    #[test]
    fn test_tag_with_no_attrs() {
        let text = "<load_tools>";
        let result = parse(text);
        assert_eq!(result.calls.len(), 1);
        assert_eq!(result.calls[0].name, "load_tools");
        assert!(result.calls[0].attrs.is_empty());
    }

    #[test]
    fn test_into_tool_call() {
        let mut attrs = HashMap::new();
        attrs.insert("path".to_string(), "C:\\src\\new.rs".to_string());
        let raw = RawToolCall {
            name: "write".to_string(),
            attrs,
            offset: 10,
        };
        let call_id = operon_context_normalize_tools::ToolCallId("call_123".to_string());
        let tool_call = raw.into_tool_call(call_id);
        assert_eq!(tool_call.id.0, "call_123");
        assert_eq!(tool_call.name, "write");
        assert_eq!(tool_call.arguments["path"], "C:\\src\\new.rs");
    }
}
