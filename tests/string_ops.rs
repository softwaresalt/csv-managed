use std::borrow::Cow;

use csv_managed::transform::string_ops::{
    camel_case, pascal_case, regex_replace, snake_case, substring,
};
use regex::Regex;

#[test]
fn camel_and_pascal_case_cover_mixed_inputs() {
    assert_eq!(camel_case("foo_bar").as_ref(), "fooBar");
    assert_eq!(pascal_case("foo_bar").as_ref(), "FooBar");
    assert_eq!(camel_case("HTTP_STATUS").as_ref(), "httpStatus");
    assert_eq!(pascal_case("multi word-value").as_ref(), "MultiWordValue");
}

#[test]
fn snake_case_reuses_when_no_change() {
    let original = "already_snake";
    let result = snake_case(original);
    assert!(matches!(result, Cow::Borrowed(_)));
}

#[test]
fn substring_handles_unicode() {
    let value = "café_price";
    let sub = substring(value, 0, 4);
    assert_eq!(sub.as_ref(), "café");
    let sub = substring(value, 5, 5);
    assert_eq!(sub.as_ref(), "price");
}

#[test]
fn regex_replace_borrows_when_no_match() {
    let regex = Regex::new("foo").unwrap();
    let value = "bar";
    let result = regex_replace(value, &regex, "baz");
    assert!(matches!(result, Cow::Borrowed(_)));
}
