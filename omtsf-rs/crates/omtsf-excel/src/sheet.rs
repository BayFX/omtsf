/// Shared helpers for reading calamine worksheet rows.
///
/// Provides column-header indexing and typed cell extraction with
/// `{Sheet}!{Column}{Row}` cell references in all errors.
use calamine::{Data, Range};

use crate::error::ImportError;

/// Builds a column-name → column-index map from the header row (row 0).
///
/// Returns a warning message for each unrecognized header if `known_headers`
/// is provided. Columns not present in `known_headers` still appear in the
/// returned map; callers may choose to warn on extra columns.
pub fn build_header_index(
    sheet: &Range<Data>,
    sheet_name: &str,
) -> Result<std::collections::HashMap<String, usize>, ImportError> {
    let mut map = std::collections::HashMap::new();
    if sheet.is_empty() {
        return Ok(map);
    }
    let first_row = sheet.rows().next();
    let Some(row) = first_row else {
        return Ok(map);
    };
    for (col_idx, cell) in row.iter().enumerate() {
        let header = cell_to_string(cell).trim().to_lowercase();
        if !header.is_empty() {
            // Warn-level: duplicate headers are silently last-one-wins.
            let _ = sheet_name; // used for context in callers
            map.insert(header, col_idx);
        }
    }
    Ok(map)
}

/// Converts a `calamine::Data` cell to a trimmed `String`.
///
/// Returns an empty string for empty, blank, or null cells.
pub fn cell_to_string(cell: &Data) -> String {
    match cell {
        Data::String(s) => s.trim().to_owned(),
        Data::Float(f) => {
            // Use integer representation when the value is whole.
            if *f == f.floor() && f.abs() < 1e15 {
                format!("{}", *f as i64)
            } else {
                f.to_string()
            }
        }
        Data::Int(i) => i.to_string(),
        Data::Bool(b) => b.to_string(),
        Data::DateTime(dt) => dt.to_string(),
        Data::DateTimeIso(s) => s.clone(),
        Data::DurationIso(s) => s.clone(),
        Data::Error(_) => String::new(),
        Data::Empty => String::new(),
    }
}

/// Returns true if a cell is empty or whitespace-only.
pub fn cell_is_empty(cell: &Data) -> bool {
    matches!(cell, Data::Empty) || cell_to_string(cell).is_empty()
}

/// Looks up the column index for `header` and returns an error if missing.
pub fn require_column(
    headers: &std::collections::HashMap<String, usize>,
    sheet_name: &str,
    header: &str,
) -> Result<usize, ImportError> {
    headers
        .get(&header.to_lowercase())
        .copied()
        .ok_or_else(|| ImportError::MissingColumn {
            sheet: sheet_name.to_owned(),
            column: header.to_owned(),
        })
}

/// Builds a column reference string like `"B"` from a zero-based column index.
pub fn col_letter(col_idx: usize) -> String {
    let mut n = col_idx + 1;
    let mut letters = Vec::new();
    while n > 0 {
        n -= 1;
        letters.push((b'A' + (n % 26) as u8) as char);
        n /= 26;
    }
    letters.iter().rev().collect()
}

/// Formats a cell reference as `"{Sheet}!{Col}{Row}"`.
///
/// `row_idx` is zero-based (row 0 = header); displayed as 1-based + 1 offset
/// to account for the header row, so data row 0 displays as row 2.
pub fn cell_ref(sheet_name: &str, col_idx: usize, data_row_idx: usize) -> String {
    format!("{}!{}{}", sheet_name, col_letter(col_idx), data_row_idx + 2)
}

/// Reads an optional string cell; returns `None` if blank.
pub fn read_optional_string(
    row: &[Data],
    headers: &std::collections::HashMap<String, usize>,
    header: &str,
) -> Option<String> {
    let col = headers.get(&header.to_lowercase()).copied()?;
    let cell = row.get(col).unwrap_or(&Data::Empty);
    let val = cell_to_string(cell);
    if val.is_empty() { None } else { Some(val) }
}

/// Reads an optional float cell; returns `None` if blank.
pub fn read_optional_float(
    row: &[Data],
    headers: &std::collections::HashMap<String, usize>,
    header: &str,
    sheet_name: &str,
    data_row_idx: usize,
) -> Result<Option<f64>, ImportError> {
    let Some(col) = headers.get(&header.to_lowercase()).copied() else {
        return Ok(None);
    };
    let cell = row.get(col).unwrap_or(&Data::Empty);
    if cell_is_empty(cell) {
        return Ok(None);
    }
    let raw = cell_to_string(cell);
    // Strip trailing percent sign if present.
    let raw = raw.trim_end_matches('%');
    raw.parse::<f64>()
        .map(Some)
        .map_err(|_| ImportError::InvalidCell {
            cell_ref: cell_ref(sheet_name, col, data_row_idx),
            expected: "numeric value".to_owned(),
            got: raw.to_owned(),
        })
}

/// Reads an optional u32 cell.
pub fn read_optional_u32(
    row: &[Data],
    headers: &std::collections::HashMap<String, usize>,
    header: &str,
    sheet_name: &str,
    data_row_idx: usize,
) -> Result<Option<u32>, ImportError> {
    let val = read_optional_float(row, headers, header, sheet_name, data_row_idx)?;
    Ok(val.map(|f| f as u32))
}
