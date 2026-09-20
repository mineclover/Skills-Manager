use serde_json::json;

/// Errors always go to stderr, in both modes.
///
/// In --json mode that keeps stdout a single JSON document: a command can print
/// its result and still exit non-zero (e.g. `fix` with failed repairs) without
/// the consumer having to parse two concatenated objects.
pub fn print_error(error: &anyhow::Error, as_json: bool) {
    if as_json {
        eprintln!("{}", json!({ "error": format!("{error:#}") }));
    } else {
        eprintln!("error: {error:#}");
    }
}
