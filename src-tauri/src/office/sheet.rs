//! Spreadsheets for the Quick (built-in) PDF: each sheet's values as an HTML table, which macOS's
//! text engine then prints with every language's characters intact. Values only: cell colours,
//! borders, charts and Excel's number formats aren't kept (15% shows as 0.15).

use std::path::Path;

use calamine::{open_workbook_auto, Data, Reader};

use crate::errors::AppError;

/// One sheet: its name and its cells, row by row.
pub struct Sheet {
    pub name: String,
    pub rows: Vec<Vec<Cell>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Cell {
    pub text: String,
    /// Numbers and dates are right-aligned, like in Excel.
    pub numeric: bool,
}

fn corrupt(err: impl std::fmt::Display) -> AppError {
    AppError::Corrupt {
        detail: err.to_string(),
    }
}

/// Every sheet (or just the first) of a spreadsheet or CSV file.
pub fn read(path: &Path, first_only: bool) -> Result<Vec<Sheet>, AppError> {
    if crate::output::extension(path) == "csv" {
        let bytes = std::fs::read(path)?;
        let text = String::from_utf8_lossy(&bytes);
        let name = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        return Ok(vec![Sheet {
            name,
            rows: parse_csv(&text),
        }]);
    }
    let mut book = open_workbook_auto(path).map_err(corrupt)?;
    let mut names = book.sheet_names();
    if first_only {
        names.truncate(1);
    }
    let mut sheets = Vec::new();
    for name in names {
        let range = book.worksheet_range(&name).map_err(corrupt)?;
        let rows = range
            .rows()
            .map(|row| row.iter().map(cell).collect())
            .collect();
        sheets.push(Sheet { name, rows });
    }
    Ok(sheets)
}

fn cell(data: &Data) -> Cell {
    let text = |t: String| Cell {
        text: t,
        numeric: false,
    };
    let number = |t: String| Cell {
        text: t,
        numeric: true,
    };
    match data {
        Data::Empty => text(String::new()),
        Data::String(s) => text(s.clone()),
        Data::Bool(b) => text(if *b { "TRUE" } else { "FALSE" }.into()),
        Data::Int(i) => number(i.to_string()),
        Data::Float(f) => number(format_number(*f)),
        Data::DateTime(dt) if dt.is_duration() => number(format_number(dt.as_f64())),
        Data::DateTime(dt) => {
            let (y, mo, d, h, mi, s, _) = dt.to_ymd_hms_milli();
            let date = format!("{y:04}-{mo:02}-{d:02}");
            number(if (h, mi, s) == (0, 0, 0) {
                date
            } else {
                format!("{date} {h:02}:{mi:02}")
            })
        }
        Data::DateTimeIso(s) | Data::DurationIso(s) => number(s.clone()),
        Data::Error(e) => text(e.to_string()),
    }
}

/// 1200.0 → "1200", 0.1 + 0.2 → "0.3": whole numbers without ".0", others to 10 significant places.
pub fn format_number(value: f64) -> String {
    if !value.is_finite() {
        return value.to_string();
    }
    if value.fract() == 0.0 && value.abs() < 1e15 {
        return format!("{}", value as i64);
    }
    let digits = (9 - value.abs().log10().floor() as i32).clamp(0, 15) as usize;
    let s = format!("{value:.digits$}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// CSV as written by Excel and Numbers: quoted fields with "" for quotes, CRLF or LF lines, and
/// semicolons instead of commas when that's clearly the separator (European exports).
pub fn parse_csv(text: &str) -> Vec<Vec<Cell>> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let first = text.lines().next().unwrap_or("");
    let sep = if first.matches(';').count() > first.matches(',').count() {
        ';'
    } else {
        ','
    };
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = text.chars().peekable();
    let push = |row: &mut Vec<Cell>, field: &mut String| {
        let t = std::mem::take(field);
        let numeric = !t.is_empty() && t.trim().parse::<f64>().is_ok();
        row.push(Cell { text: t, numeric });
    };
    while let Some(c) = chars.next() {
        match c {
            '"' if quoted && chars.peek() == Some(&'"') => {
                field.push('"');
                chars.next();
            }
            '"' => quoted = !quoted,
            c if c == sep && !quoted => push(&mut row, &mut field),
            '\r' if !quoted => {}
            '\n' if !quoted => {
                push(&mut row, &mut field);
                rows.push(std::mem::take(&mut row));
            }
            c => field.push(c),
        }
    }
    if !field.is_empty() || !row.is_empty() {
        push(&mut row, &mut field);
        rows.push(row);
    }
    rows
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// The sheets as one HTML page: a bordered table per sheet, under its name when `titled` (the file
/// has several sheets). Empty trailing columns are dropped.
pub fn to_html(sheets: &[Sheet], titled: bool) -> String {
    let mut html = String::from(
        "<!doctype html><html><head><meta charset=\"utf-8\"><style>\
         body{font-family:-apple-system,Helvetica,sans-serif;font-size:9pt}\
         h2{font-size:12pt;margin:0 0 6pt}\
         table{border-collapse:collapse}\
         td{border:0.5pt solid #9a9a9a;padding:2pt 4pt;vertical-align:top}\
         td.n{text-align:right}\
         </style></head><body>",
    );
    for (i, sheet) in sheets.iter().enumerate() {
        if i > 0 {
            html.push_str("<p></p>");
        }
        if titled {
            html.push_str(&format!("<h2>{}</h2>", escape(&sheet.name)));
        }
        let width = sheet
            .rows
            .iter()
            .map(|r| {
                r.iter()
                    .rposition(|c| !c.text.is_empty())
                    .map_or(0, |p| p + 1)
            })
            .max()
            .unwrap_or(0);
        if width == 0 {
            html.push_str("<p>(empty sheet)</p>");
            continue;
        }
        // The text engine sizes columns too tightly (words wrap mid-way), so each column gets a
        // width from its longest entry: at least 3, at most 30 characters wide.
        let widths: Vec<f64> = (0..width)
            .map(|c| {
                let longest = sheet
                    .rows
                    .iter()
                    .filter_map(|r| r.get(c))
                    .flat_map(|cell| cell.text.lines().map(|l| l.chars().count()))
                    .max()
                    .unwrap_or(0);
                (longest as f64 * 0.62 + 1.0).clamp(3.0, 30.0)
            })
            .collect();
        html.push_str("<table>");
        for (r, row) in sheet.rows.iter().enumerate() {
            html.push_str("<tr>");
            for (c, w) in widths.iter().enumerate() {
                let size = if r == 0 {
                    format!(" style=\"width:{w:.1}em\"")
                } else {
                    String::new()
                };
                let class = if row.get(c).is_some_and(|cell| cell.numeric) {
                    " class=\"n\""
                } else {
                    ""
                };
                let text = row.get(c).map_or(String::new(), |cell| {
                    escape(&cell.text).replace('\n', "<br>")
                });
                html.push_str(&format!("<td{class}{size}>{text}</td>"));
            }
            html.push_str("</tr>");
        }
        html.push_str("</table>");
    }
    html.push_str("</body></html>");
    html
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(rows: &[Vec<Cell>]) -> Vec<Vec<&str>> {
        rows.iter()
            .map(|r| r.iter().map(|c| c.text.as_str()).collect())
            .collect()
    }

    #[test]
    fn csv_quotes_newlines_and_semicolons() {
        let rows = parse_csv("\u{feff}Item,Note,Amount\r\n\"Rent, flat\",\"He said \"\"ok\"\"\",1200\nFood,\"two\nlines\",45.5\n");
        assert_eq!(
            texts(&rows),
            [
                vec!["Item", "Note", "Amount"],
                vec!["Rent, flat", "He said \"ok\"", "1200"],
                vec!["Food", "two\nlines", "45.5"],
            ]
        );
        assert!(rows[1][2].numeric && !rows[1][0].numeric);
        let euro = parse_csv("Artikel;Preis\nBrot;2,50\n");
        assert_eq!(
            texts(&euro),
            [vec!["Artikel", "Preis"], vec!["Brot", "2,50"]]
        );
    }

    #[test]
    fn numbers_read_naturally() {
        assert_eq!(format_number(1200.0), "1200");
        assert_eq!(format_number(0.1 + 0.2), "0.3");
        assert_eq!(format_number(-45.5), "-45.5");
        assert_eq!(format_number(1234567.891), "1234567.891");
        assert_eq!(format_number(0.000125), "0.000125");
    }

    #[test]
    fn html_escapes_text_and_keeps_any_language() {
        let sheet = |name: &str, rows: Vec<Vec<&str>>| Sheet {
            name: name.into(),
            rows: rows
                .into_iter()
                .map(|r| {
                    r.into_iter()
                        .map(|t| Cell {
                            text: t.into(),
                            numeric: t.parse::<f64>().is_ok(),
                        })
                        .collect()
                })
                .collect(),
        };
        let html = to_html(
            &[
                sheet(
                    "खर्च <2026>",
                    vec![vec!["किराया", "1200", ""], vec!["Café & co", "45", ""]],
                ),
                sheet("Empty", vec![]),
            ],
            true,
        );
        assert!(html.contains("<h2>खर्च &lt;2026&gt;</h2>"));
        assert!(
            html.contains("<td>Café &amp; co</td><td class=\"n\">45</td></tr>"),
            "empty last column dropped"
        );
        assert!(html.contains("(empty sheet)"));
    }

    #[test]
    fn reads_a_real_spreadsheet() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/budget.xlsx");
        let sheets = read(&path, false).unwrap();
        assert_eq!(sheets.len(), 2);
        assert_eq!(sheets[0].name, "Budget");
        assert_eq!(sheets[0].rows[0][0].text, "Item");
        assert_eq!(sheets[0].rows[1][1].text, "1200");
        assert!(sheets[0].rows[1][1].numeric);
        assert_eq!(sheets[0].rows[3][0].text, "किराया");
        assert_eq!(read(&path, true).unwrap().len(), 1);
    }
}
