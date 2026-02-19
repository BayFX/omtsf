/// Diagnostic formatting: human-readable and JSON (NDJSON) modes.
///
/// This module implements two output strategies for [`omtsf_core::Diagnostic`]
/// values:
///
/// - **Human mode** (default): one line per diagnostic, color-coded by
///   severity to stderr. Colors are disabled when `--no-color` is set, the
///   `NO_COLOR` environment variable is present (per <https://no-color.org>),
///   or stderr is not a TTY.
/// - **JSON mode**: each diagnostic is serialized as a single-line JSON object
///   (NDJSON) to stderr.
///
/// Both modes support a **quiet** flag (suppress non-error diagnostics and
/// summary) and a **verbose** flag (add timing and metadata).
use std::io::{IsTerminal as _, Write};
use std::time::Duration;

use omtsf_core::{Diagnostic, Severity};

// ---------------------------------------------------------------------------
// Color support detection
// ---------------------------------------------------------------------------

/// Returns `true` if ANSI color codes should be emitted to stderr.
///
/// Colors are disabled when any of the following conditions hold:
/// - `no_color_flag` is `true` (the `--no-color` CLI flag was passed).
/// - The `NO_COLOR` environment variable is present (any non-empty value).
/// - stderr is not a TTY (e.g. the output is piped to a file).
pub fn colors_enabled(no_color_flag: bool) -> bool {
    if no_color_flag {
        return false;
    }
    // NO_COLOR env var: presence of the variable (any value) disables color.
    if std::env::var_os("NO_COLOR").is_some() {
        return false;
    }
    // Check whether stderr is a TTY.
    std::io::stderr().is_terminal()
}

// ---------------------------------------------------------------------------
// ANSI escape sequences
// ---------------------------------------------------------------------------

const ANSI_RED: &str = "\x1b[31m";
const ANSI_YELLOW: &str = "\x1b[33m";
const ANSI_CYAN: &str = "\x1b[36m";
const ANSI_RESET: &str = "\x1b[0m";

// ---------------------------------------------------------------------------
// FormatterConfig
// ---------------------------------------------------------------------------

/// Configuration for the diagnostic formatter, derived from CLI flags.
#[derive(Debug, Clone)]
pub struct FormatterConfig {
    /// Whether ANSI colors are enabled.
    pub colors: bool,
    /// Suppress all non-error stderr output.
    pub quiet: bool,
    /// Emit timing and metadata to stderr.
    pub verbose: bool,
}

impl FormatterConfig {
    /// Constructs a [`FormatterConfig`] from the raw CLI flags.
    ///
    /// `no_color_flag` is the `--no-color` boolean. Color detection also
    /// checks the `NO_COLOR` env var and the stderr TTY state.
    pub fn from_flags(no_color_flag: bool, quiet: bool, verbose: bool) -> Self {
        Self {
            colors: colors_enabled(no_color_flag),
            quiet,
            verbose,
        }
    }
}

// ---------------------------------------------------------------------------
// Human-mode formatting
// ---------------------------------------------------------------------------

/// Writes a single [`Diagnostic`] to `writer` in human-readable format.
///
/// Format: `[E] L1-GDM-03  edge "e-042": target "node-999" not found`
///
/// The severity tag (`[E]`, `[W]`, `[I]`) is color-coded when
/// `config.colors` is `true`:
/// - `[E]` → red
/// - `[W]` → yellow
/// - `[I]` → cyan
///
/// In quiet mode, [`Severity::Warning`] and [`Severity::Info`] diagnostics
/// are suppressed.
///
/// # Errors
///
/// Returns an error only if writing to `writer` fails.
pub fn write_diagnostic_human<W: Write>(
    writer: &mut W,
    diag: &Diagnostic,
    config: &FormatterConfig,
) -> std::io::Result<()> {
    // Quiet mode: suppress warnings and info.
    if config.quiet {
        match diag.severity {
            Severity::Warning | Severity::Info => return Ok(()),
            Severity::Error => {}
        }
    }

    let (tag, color) = match diag.severity {
        Severity::Error => ("[E]", ANSI_RED),
        Severity::Warning => ("[W]", ANSI_YELLOW),
        Severity::Info => ("[I]", ANSI_CYAN),
    };

    if config.colors {
        writeln!(
            writer,
            "{color}{tag}{ANSI_RESET} {rule_id}  {location}: {message}",
            rule_id = diag.rule_id,
            location = diag.location,
            message = diag.message,
        )
    } else {
        writeln!(
            writer,
            "{tag} {rule_id}  {location}: {message}",
            rule_id = diag.rule_id,
            location = diag.location,
            message = diag.message,
        )
    }
}

/// Writes a summary line to `writer` for human mode.
///
/// Format: `3 errors, 1 warning, 0 info`
///
/// In quiet mode the summary is suppressed. In verbose mode the checked
/// entity counts are also printed if provided.
///
/// # Errors
///
/// Returns an error only if writing to `writer` fails.
pub fn write_summary_human<W: Write>(
    writer: &mut W,
    errors: usize,
    warnings: usize,
    infos: usize,
    config: &FormatterConfig,
) -> std::io::Result<()> {
    if config.quiet {
        return Ok(());
    }
    writeln!(
        writer,
        "{errors} {}, {warnings} {}, {infos} {}",
        pluralize(errors, "error", "errors"),
        pluralize(warnings, "warning", "warnings"),
        pluralize(infos, "info", "info"),
    )
}

/// Writes timing information to `writer` in verbose mode.
///
/// This is a no-op when `config.verbose` is `false`.
///
/// # Errors
///
/// Returns an error only if writing to `writer` fails.
pub fn write_timing_human<W: Write>(
    writer: &mut W,
    label: &str,
    duration: Duration,
    config: &FormatterConfig,
) -> std::io::Result<()> {
    if !config.verbose {
        return Ok(());
    }
    writeln!(writer, "{label} in {}ms", duration.as_millis())
}

// ---------------------------------------------------------------------------
// JSON-mode formatting (NDJSON)
// ---------------------------------------------------------------------------

/// Writes a single [`Diagnostic`] to `writer` as a NDJSON line.
///
/// Each line is a self-contained JSON object:
/// ```json
/// {"rule_id":"L1-GDM-03","severity":"error","location":"edge \"e-042\"","message":"..."}
/// ```
///
/// In quiet mode, [`Severity::Warning`] and [`Severity::Info`] diagnostics
/// are suppressed.
///
/// # Errors
///
/// Returns an error only if writing to `writer` fails.
pub fn write_diagnostic_json<W: Write>(
    writer: &mut W,
    diag: &Diagnostic,
    config: &FormatterConfig,
) -> std::io::Result<()> {
    // Quiet mode: suppress warnings and info.
    if config.quiet {
        match diag.severity {
            Severity::Warning | Severity::Info => return Ok(()),
            Severity::Error => {}
        }
    }

    let severity_str = match diag.severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Info => "info",
    };

    // Build a minimal JSON object without pulling in serde_json for the CLI
    // formatter. We use manual JSON serialization to keep the dependency
    // surface small and avoid allocating a full serde_json::Value.
    let rule_id_json = json_string(diag.rule_id.code());
    let severity_json = json_string(severity_str);
    let location_json = json_string(&diag.location.to_string());
    let message_json = json_string(&diag.message);

    writeln!(
        writer,
        r#"{{"rule_id":{rule_id_json},"severity":{severity_json},"location":{location_json},"message":{message_json}}}"#,
    )
}

/// Writes a JSON summary object as a final NDJSON line.
///
/// Format: `{"summary":{"errors":3,"warnings":1,"info":0}}`
///
/// In quiet mode the summary is suppressed.
///
/// # Errors
///
/// Returns an error only if writing to `writer` fails.
pub fn write_summary_json<W: Write>(
    writer: &mut W,
    errors: usize,
    warnings: usize,
    infos: usize,
    config: &FormatterConfig,
) -> std::io::Result<()> {
    if config.quiet {
        return Ok(());
    }
    writeln!(
        writer,
        r#"{{"summary":{{"errors":{errors},"warnings":{warnings},"info":{infos}}}}}"#,
    )
}

// ---------------------------------------------------------------------------
// Helper: dispatch by format
// ---------------------------------------------------------------------------

/// Output format selection, mirroring the CLI `--format` flag.
///
/// Used by [`write_diagnostic`] and [`write_summary`] to dispatch to the
/// correct formatter without the caller needing to know the format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormatMode {
    /// Human-readable, optionally colored output.
    Human,
    /// Structured NDJSON output.
    Json,
}

/// Writes a single [`Diagnostic`] to `writer` in the requested format.
///
/// This is a convenience dispatcher over [`write_diagnostic_human`] and
/// [`write_diagnostic_json`].
///
/// # Errors
///
/// Returns an error only if writing to `writer` fails.
pub fn write_diagnostic<W: Write>(
    writer: &mut W,
    diag: &Diagnostic,
    mode: FormatMode,
    config: &FormatterConfig,
) -> std::io::Result<()> {
    match mode {
        FormatMode::Human => write_diagnostic_human(writer, diag, config),
        FormatMode::Json => write_diagnostic_json(writer, diag, config),
    }
}

/// Writes a summary to `writer` in the requested format.
///
/// # Errors
///
/// Returns an error only if writing to `writer` fails.
pub fn write_summary<W: Write>(
    writer: &mut W,
    errors: usize,
    warnings: usize,
    infos: usize,
    mode: FormatMode,
    config: &FormatterConfig,
) -> std::io::Result<()> {
    match mode {
        FormatMode::Human => write_summary_human(writer, errors, warnings, infos, config),
        FormatMode::Json => write_summary_json(writer, errors, warnings, infos, config),
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Returns the singular or plural form of `word` depending on `count`.
fn pluralize<'a>(count: usize, singular: &'a str, plural: &'a str) -> &'a str {
    if count == 1 { singular } else { plural }
}

/// Serializes `s` as a JSON string literal, escaping special characters.
///
/// Handles `"`, `\`, and the ASCII control characters `\n`, `\r`, `\t`.
fn json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str(r#"\""#),
            '\\' => out.push_str(r"\\"),
            '\n' => out.push_str(r"\n"),
            '\r' => out.push_str(r"\r"),
            '\t' => out.push_str(r"\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use omtsf_core::{Diagnostic, Location, RuleId, Severity};

    use super::*;

    // ── helpers ──────────────────────────────────────────────────────────────

    fn no_color_config() -> FormatterConfig {
        FormatterConfig {
            colors: false,
            quiet: false,
            verbose: false,
        }
    }

    fn quiet_config() -> FormatterConfig {
        FormatterConfig {
            colors: false,
            quiet: true,
            verbose: false,
        }
    }

    fn verbose_config() -> FormatterConfig {
        FormatterConfig {
            colors: false,
            quiet: false,
            verbose: true,
        }
    }

    fn make_error() -> Diagnostic {
        Diagnostic::new(
            RuleId::L1Gdm03,
            Severity::Error,
            Location::Edge {
                edge_id: "e-042".to_owned(),
                field: Some("target".to_owned()),
            },
            "target \"node-999\" not found",
        )
    }

    fn make_warning() -> Diagnostic {
        Diagnostic::new(
            RuleId::L2Eid01,
            Severity::Warning,
            Location::Node {
                node_id: "org-001".to_owned(),
                field: None,
            },
            "no external identifiers",
        )
    }

    fn make_info() -> Diagnostic {
        Diagnostic::new(
            RuleId::L3Eid02,
            Severity::Info,
            Location::Node {
                node_id: "org-001".to_owned(),
                field: None,
            },
            "LEI status is LAPSED",
        )
    }

    fn capture_human(diag: &Diagnostic, config: &FormatterConfig) -> String {
        let mut buf: Vec<u8> = Vec::new();
        write_diagnostic_human(&mut buf, diag, config).expect("write");
        String::from_utf8(buf).expect("utf8")
    }

    fn capture_json(diag: &Diagnostic, config: &FormatterConfig) -> String {
        let mut buf: Vec<u8> = Vec::new();
        write_diagnostic_json(&mut buf, diag, config).expect("write");
        String::from_utf8(buf).expect("utf8")
    }

    // ── human format ─────────────────────────────────────────────────────────

    #[test]
    fn human_error_contains_tag_rule_location_message() {
        let s = capture_human(&make_error(), &no_color_config());
        assert!(s.starts_with("[E]"), "output: {s}");
        assert!(s.contains("L1-GDM-03"), "output: {s}");
        assert!(s.contains("e-042"), "output: {s}");
        assert!(s.contains("node-999"), "output: {s}");
    }

    #[test]
    fn human_warning_contains_w_tag() {
        let s = capture_human(&make_warning(), &no_color_config());
        assert!(s.starts_with("[W]"), "output: {s}");
        assert!(s.contains("L2-EID-01"), "output: {s}");
    }

    #[test]
    fn human_info_contains_i_tag() {
        let s = capture_human(&make_info(), &no_color_config());
        assert!(s.starts_with("[I]"), "output: {s}");
        assert!(s.contains("L3-EID-02"), "output: {s}");
    }

    #[test]
    fn human_color_wraps_tag_with_ansi() {
        let config = FormatterConfig {
            colors: true,
            quiet: false,
            verbose: false,
        };
        let s = capture_human(&make_error(), &config);
        // The error tag should be wrapped in red ANSI codes.
        assert!(s.contains(ANSI_RED), "no red ANSI: {s}");
        assert!(s.contains(ANSI_RESET), "no reset ANSI: {s}");
    }

    #[test]
    fn human_color_warning_uses_yellow() {
        let config = FormatterConfig {
            colors: true,
            quiet: false,
            verbose: false,
        };
        let s = capture_human(&make_warning(), &config);
        assert!(s.contains(ANSI_YELLOW), "no yellow ANSI: {s}");
    }

    #[test]
    fn human_color_info_uses_cyan() {
        let config = FormatterConfig {
            colors: true,
            quiet: false,
            verbose: false,
        };
        let s = capture_human(&make_info(), &config);
        assert!(s.contains(ANSI_CYAN), "no cyan ANSI: {s}");
    }

    #[test]
    fn human_quiet_suppresses_warning() {
        let mut buf: Vec<u8> = Vec::new();
        write_diagnostic_human(&mut buf, &make_warning(), &quiet_config()).expect("write");
        assert!(buf.is_empty(), "warning should be suppressed in quiet mode");
    }

    #[test]
    fn human_quiet_suppresses_info() {
        let mut buf: Vec<u8> = Vec::new();
        write_diagnostic_human(&mut buf, &make_info(), &quiet_config()).expect("write");
        assert!(buf.is_empty(), "info should be suppressed in quiet mode");
    }

    #[test]
    fn human_quiet_keeps_error() {
        let s = capture_human(&make_error(), &quiet_config());
        assert!(
            !s.is_empty(),
            "error should not be suppressed in quiet mode"
        );
        assert!(s.contains("[E]"), "output: {s}");
    }

    // ── human summary ────────────────────────────────────────────────────────

    #[test]
    fn human_summary_format() {
        let mut buf: Vec<u8> = Vec::new();
        write_summary_human(&mut buf, 3, 1, 0, &no_color_config()).expect("write");
        let s = String::from_utf8(buf).expect("utf8");
        assert!(s.contains("3 errors"), "output: {s}");
        assert!(s.contains("1 warning"), "output: {s}");
        assert!(s.contains("0 info"), "output: {s}");
    }

    #[test]
    fn human_summary_singular_error() {
        let mut buf: Vec<u8> = Vec::new();
        write_summary_human(&mut buf, 1, 0, 0, &no_color_config()).expect("write");
        let s = String::from_utf8(buf).expect("utf8");
        assert!(s.contains("1 error"), "output: {s}");
        assert!(!s.contains("1 errors"), "output: {s}");
    }

    #[test]
    fn human_summary_suppressed_in_quiet_mode() {
        let mut buf: Vec<u8> = Vec::new();
        write_summary_human(&mut buf, 3, 1, 0, &quiet_config()).expect("write");
        assert!(buf.is_empty(), "summary should be suppressed in quiet mode");
    }

    // ── verbose timing ───────────────────────────────────────────────────────

    #[test]
    fn verbose_timing_emitted_when_verbose() {
        let mut buf: Vec<u8> = Vec::new();
        write_timing_human(
            &mut buf,
            "parsed",
            Duration::from_millis(42),
            &verbose_config(),
        )
        .expect("write");
        let s = String::from_utf8(buf).expect("utf8");
        assert!(s.contains("42ms"), "output: {s}");
        assert!(s.contains("parsed"), "output: {s}");
    }

    #[test]
    fn verbose_timing_suppressed_when_not_verbose() {
        let mut buf: Vec<u8> = Vec::new();
        write_timing_human(
            &mut buf,
            "parsed",
            Duration::from_millis(42),
            &no_color_config(),
        )
        .expect("write");
        assert!(
            buf.is_empty(),
            "timing should be suppressed when not verbose"
        );
    }

    // ── JSON format ──────────────────────────────────────────────────────────

    #[test]
    fn json_output_is_valid_ndjson_line() {
        let s = capture_json(&make_error(), &no_color_config());
        // Must be a single line ending with newline.
        let trimmed = s.trim_end_matches('\n');
        assert!(!trimmed.contains('\n'), "must be single line: {s}");
        // Must start and end with braces (JSON object).
        assert!(trimmed.starts_with('{'), "output: {s}");
        assert!(trimmed.ends_with('}'), "output: {s}");
    }

    #[test]
    fn json_error_contains_required_fields() {
        let s = capture_json(&make_error(), &no_color_config());
        assert!(s.contains("\"rule_id\""), "output: {s}");
        assert!(s.contains("\"severity\""), "output: {s}");
        assert!(s.contains("\"location\""), "output: {s}");
        assert!(s.contains("\"message\""), "output: {s}");
    }

    #[test]
    fn json_error_severity_is_error_string() {
        let s = capture_json(&make_error(), &no_color_config());
        assert!(s.contains("\"error\""), "output: {s}");
    }

    #[test]
    fn json_warning_severity_is_warning_string() {
        let s = capture_json(&make_warning(), &no_color_config());
        assert!(s.contains("\"warning\""), "output: {s}");
    }

    #[test]
    fn json_info_severity_is_info_string() {
        let s = capture_json(&make_info(), &no_color_config());
        assert!(s.contains("\"info\""), "output: {s}");
    }

    #[test]
    fn json_quiet_suppresses_warning() {
        let mut buf: Vec<u8> = Vec::new();
        write_diagnostic_json(&mut buf, &make_warning(), &quiet_config()).expect("write");
        assert!(buf.is_empty(), "warning should be suppressed in quiet mode");
    }

    #[test]
    fn json_quiet_keeps_error() {
        let s = capture_json(&make_error(), &quiet_config());
        assert!(!s.is_empty(), "error should not be suppressed");
    }

    // ── JSON summary ─────────────────────────────────────────────────────────

    #[test]
    fn json_summary_format() {
        let mut buf: Vec<u8> = Vec::new();
        write_summary_json(&mut buf, 3, 1, 0, &no_color_config()).expect("write");
        let s = String::from_utf8(buf).expect("utf8");
        assert!(s.contains("\"summary\""), "output: {s}");
        assert!(s.contains("\"errors\":3"), "output: {s}");
        assert!(s.contains("\"warnings\":1"), "output: {s}");
        assert!(s.contains("\"info\":0"), "output: {s}");
    }

    #[test]
    fn json_summary_suppressed_in_quiet_mode() {
        let mut buf: Vec<u8> = Vec::new();
        write_summary_json(&mut buf, 3, 1, 0, &quiet_config()).expect("write");
        assert!(buf.is_empty(), "summary should be suppressed in quiet mode");
    }

    // ── json_string escaping ─────────────────────────────────────────────────

    #[test]
    fn json_string_escapes_double_quote() {
        assert_eq!(json_string(r#"say "hi""#), r#""say \"hi\"""#);
    }

    #[test]
    fn json_string_escapes_backslash() {
        assert_eq!(json_string(r"a\b"), r#""a\\b""#);
    }

    #[test]
    fn json_string_escapes_newline() {
        assert_eq!(json_string("a\nb"), r#""a\nb""#);
    }

    #[test]
    fn json_string_plain_ascii() {
        assert_eq!(json_string("hello"), r#""hello""#);
    }

    // ── colors_enabled logic ─────────────────────────────────────────────────

    #[test]
    fn colors_disabled_by_no_color_flag() {
        // Temporarily unset NO_COLOR to test only the flag path.
        // We can only test the flag=true case reliably in unit tests.
        assert!(
            !colors_enabled(true),
            "colors should be off when flag is set"
        );
    }

    // ── FormatMode dispatch ───────────────────────────────────────────────────

    #[test]
    fn write_diagnostic_human_mode_dispatches_correctly() {
        let mut buf: Vec<u8> = Vec::new();
        write_diagnostic(
            &mut buf,
            &make_error(),
            FormatMode::Human,
            &no_color_config(),
        )
        .expect("write");
        let s = String::from_utf8(buf).expect("utf8");
        assert!(s.starts_with("[E]"), "output: {s}");
    }

    #[test]
    fn write_diagnostic_json_mode_dispatches_correctly() {
        let mut buf: Vec<u8> = Vec::new();
        write_diagnostic(
            &mut buf,
            &make_error(),
            FormatMode::Json,
            &no_color_config(),
        )
        .expect("write");
        let s = String::from_utf8(buf).expect("utf8");
        assert!(s.contains("\"rule_id\""), "output: {s}");
    }

    #[test]
    fn write_summary_human_mode_dispatches_correctly() {
        let mut buf: Vec<u8> = Vec::new();
        write_summary(&mut buf, 1, 0, 0, FormatMode::Human, &no_color_config()).expect("write");
        let s = String::from_utf8(buf).expect("utf8");
        assert!(s.contains("error"), "output: {s}");
        assert!(!s.contains("\"summary\""), "should not be JSON: {s}");
    }

    #[test]
    fn write_summary_json_mode_dispatches_correctly() {
        let mut buf: Vec<u8> = Vec::new();
        write_summary(&mut buf, 1, 0, 0, FormatMode::Json, &no_color_config()).expect("write");
        let s = String::from_utf8(buf).expect("utf8");
        assert!(s.contains("\"summary\""), "output: {s}");
    }

    // ── pluralize ────────────────────────────────────────────────────────────

    #[test]
    fn pluralize_one_uses_singular() {
        assert_eq!(pluralize(1, "error", "errors"), "error");
    }

    #[test]
    fn pluralize_zero_uses_plural() {
        assert_eq!(pluralize(0, "error", "errors"), "errors");
    }

    #[test]
    fn pluralize_many_uses_plural() {
        assert_eq!(pluralize(5, "error", "errors"), "errors");
    }
}
