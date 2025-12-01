use std::borrow::Cow;
use std::fmt::Write as _;

pub fn render_table(headers: &[String], rows: &[Vec<String>]) -> String {
    let column_count = headers.len();
    let mut widths = headers.iter().map(|h| display_width(h)).collect::<Vec<_>>();

    for row in rows {
        for (idx, cell) in row.iter().enumerate().take(column_count) {
            widths[idx] = widths[idx].max(display_width(cell));
        }
    }

    for width in &mut widths {
        *width = (*width).max(1);
    }

    let mut output = String::new();

    // Header
    let header_line = format_row(headers, &widths);
    let _ = writeln!(output, "{header_line}");

    // Separator
    let separator_widths = widths.iter().map(|w| (*w).max(3)).collect::<Vec<usize>>();
    let separator_cells = separator_widths
        .iter()
        .map(|w| "-".repeat(*w))
        .collect::<Vec<_>>();
    let separator_line = format_row(&separator_cells, &separator_widths);
    let _ = writeln!(output, "{separator_line}");

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
    let mut cells = Vec::with_capacity(values.len());
    for (idx, value) in values.iter().enumerate() {
        if idx >= widths.len() {
            break;
        }
        let sanitized = sanitize_cell(value);
        let display = display_width(sanitized.as_ref());
        let mut cell = sanitized.into_owned();
        let padding = widths
            .get(idx)
            .copied()
            .unwrap_or_default()
            .saturating_sub(display);
        if padding > 0 {
            cell.push_str(&" ".repeat(padding));
        }
        cells.push(cell);
    }
    let mut line = cells.join("  ");
    while line.ends_with(' ') {
        line.pop();
    }
    line
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

fn sanitize_cell(value: &str) -> Cow<'_, str> {
    if value.contains(['\n', '\r', '\t']) {
        let mut sanitized = String::with_capacity(value.len());
        for ch in value.chars() {
            match ch {
                '\n' | '\r' | '\t' => sanitized.push(' '),
                other => sanitized.push(other),
            }
        }
        Cow::Owned(sanitized)
    } else {
        Cow::Borrowed(value)
    }
}
