use std::fmt::Write as _;

pub fn render_table(headers: &[String], rows: &[Vec<String>]) -> String {
    let column_count = headers.len();
    let mut widths = headers.iter().map(|h| display_width(h)).collect::<Vec<_>>();

    for row in rows {
        for (idx, cell) in row.iter().enumerate().take(column_count) {
            widths[idx] = widths[idx].max(display_width(cell));
        }
    }

    let mut output = String::new();

    // Header
    let header_line = format_row(headers, &widths);
    let _ = writeln!(output, "{header_line}");

    // Separator
    let separator = widths
        .iter()
        .map(|w| "-".repeat(*w.max(&3) + 2))
        .collect::<Vec<_>>()
        .join("+");
    let _ = writeln!(output, "{separator}");

    // Rows
    for row in rows {
        let row_line = format_row(row, &widths);
        let _ = writeln!(output, "{row_line}");
    }

    output
}

pub fn print_table(headers: &[String], rows: &[Vec<String>]) {
    let rendered = render_table(headers, rows);
    print!("{rendered}");
}

fn format_row(values: &[String], widths: &[usize]) -> String {
    values
        .iter()
        .enumerate()
        .map(|(idx, value)| format!(" {:<width$} ", value, width = widths[idx]))
        .collect::<Vec<_>>()
        .join("|")
}

fn display_width(value: &str) -> usize {
    let mut width = 0usize;
    let mut chars = value.chars();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' {
            // Skip ANSI escape sequence (e.g. \x1b[31m)
            for next in chars.by_ref() {
                if next == 'm' {
                    break;
                }
            }
        } else {
            width += 1;
        }
    }
    width
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_table_aligns_columns() {
        let headers = vec!["id".to_string(), "name".to_string()];
        let rows = vec![
            vec!["1".to_string(), "Alice".to_string()],
            vec!["2".to_string(), "Bob".to_string()],
        ];

        let rendered = render_table(&headers, &rows);
        let lines: Vec<&str> = rendered.lines().collect();

        assert_eq!(lines.len(), 4);
        assert_eq!(lines[0], " id | name  ");
        assert!(lines[1].contains("---"));
        assert_eq!(lines[2], " 1  | Alice ");
        assert_eq!(lines[3], " 2  | Bob   ");
    }

    #[test]
    fn display_width_counts_characters() {
        assert_eq!(display_width("abc"), 3);
        assert_eq!(display_width(""), 0);
        assert_eq!(display_width("résumé"), 6);
    }

    #[test]
    fn display_width_ignores_ansi_sequences() {
        let value = "\u{1b}[31minvalid\u{1b}[0m";
        assert_eq!(display_width(value), "invalid".len());
    }
}
