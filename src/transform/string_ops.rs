use std::borrow::Cow;

use heck::{ToLowerCamelCase, ToSnakeCase, ToUpperCamelCase};
use regex::Regex;

/// Returns a lowercase representation, reusing the original string if already lowercase.
pub fn lowercase(input: &str) -> Cow<'_, str> {
    if input.chars().all(|ch| !ch.is_uppercase()) {
        Cow::Borrowed(input)
    } else {
        Cow::Owned(input.to_lowercase())
    }
}

/// Returns an uppercase representation, avoiding allocation when unnecessary.
pub fn uppercase(input: &str) -> Cow<'_, str> {
    if input.chars().all(|ch| !ch.is_lowercase()) {
        Cow::Borrowed(input)
    } else {
        Cow::Owned(input.to_uppercase())
    }
}

/// Trims leading/trailing whitespace while borrowing the original when unchanged.
pub fn trim(input: &str) -> Cow<'_, str> {
    let trimmed = input.trim();
    if trimmed.len() == input.len() {
        Cow::Borrowed(input)
    } else {
        Cow::Borrowed(trimmed)
    }
}

/// Converts identifiers to `snake_case`.
pub fn snake_case(input: &str) -> Cow<'_, str> {
    let converted = input.to_snake_case();
    if converted == input {
        Cow::Borrowed(input)
    } else {
        Cow::Owned(converted)
    }
}

/// Converts identifiers to `camelCase`.
pub fn camel_case(input: &str) -> Cow<'_, str> {
    let converted = input.to_lower_camel_case();
    if converted == input {
        Cow::Borrowed(input)
    } else {
        Cow::Owned(converted)
    }
}

/// Converts identifiers to `PascalCase`.
pub fn pascal_case(input: &str) -> Cow<'_, str> {
    let converted = input.to_upper_camel_case();
    if converted == input {
        Cow::Borrowed(input)
    } else {
        Cow::Owned(converted)
    }
}

/// Returns a substring using character indices to stay UTF-8 safe.
pub fn substring<'a>(value: &'a str, start: usize, length: usize) -> Cow<'a, str> {
    if length == 0 {
        return Cow::Owned(String::new());
    }
    let mut char_index = 0usize;
    let mut start_byte: Option<usize> = None;
    let mut end_byte: Option<usize> = None;
    let end_target = start.saturating_add(length);

    for (idx, _) in value.char_indices() {
        if char_index == start {
            start_byte = Some(idx);
        }
        if char_index == end_target {
            end_byte = Some(idx);
            break;
        }
        char_index += 1;
    }

    if start >= char_index && start_byte.is_none() {
        return Cow::Owned(String::new());
    }

    let start_byte = start_byte.unwrap_or_else(|| value.len());
    let end_byte = end_byte.unwrap_or_else(|| value.len());
    if start_byte >= end_byte {
        return Cow::Owned(String::new());
    }
    if start_byte == 0 && end_byte == value.len() {
        return Cow::Borrowed(value);
    }
    Cow::Borrowed(&value[start_byte..end_byte])
}

/// Applies a regex replacement while avoiding allocation when there are no matches.
pub fn regex_replace<'a>(
    value: &'a str,
    regex: &Regex,
    replacement: &str,
) -> Cow<'a, str> {
    if regex.is_match(value) {
        Cow::Owned(regex.replace_all(value, replacement).to_string())
    } else {
        Cow::Borrowed(value)
    }
}


