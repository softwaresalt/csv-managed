use csv_managed::table::render_table;

#[test]
fn render_table_aligns_columns() {
    let headers = vec!["id".to_string(), "name".to_string()];
    let rows = vec![
        vec!["1".to_string(), "Alice".to_string()],
        vec!["2".to_string(), "Bob".to_string()],
    ];

    let rendered = render_table(&headers, &rows);
    let lines: Vec<&str> = rendered.lines().collect();

    assert_eq!(lines, vec!["id  name", "---  -----", "1   Alice", "2   Bob"]);
}

#[test]
fn render_table_normalizes_control_characters() {
    let headers = vec!["note".to_string()];
    let rows = vec![vec!["line1\nline2\tvalue".to_string()]];

    let rendered = render_table(&headers, &rows);
    let lines: Vec<&str> = rendered.lines().collect();

    assert_eq!(lines.len(), 3);
    assert_eq!(lines[2], "line1 line2 value");
}

#[test]
fn render_table_handles_unicode_and_ansi_widths() {
    let headers = vec!["résumé".to_string(), "status".to_string()];
    let rows = vec![vec!["café".to_string(), "\u{1b}[31mERR\u{1b}[0m".to_string()]];

    let rendered = render_table(&headers, &rows);
    let lines: Vec<&str> = rendered.lines().collect();

    assert_eq!(lines[0], "résumé  status");
    // "résumé" is two display columns wider than "café", so expect two padding
    // spaces plus the standard two-column separator.
    assert_eq!(lines[2], "café    \u{1b}[31mERR\u{1b}[0m");
}
