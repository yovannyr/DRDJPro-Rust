/// Reduce generated stack frames to function names and basename/source locations.
/// Arbitrary panic payloads are deliberately not captured: they can contain secrets.
pub(super) fn sanitize_stack(input: &str) -> String {
    input
        .lines()
        .take(256)
        .map(|line| {
            if !line.contains(['/', '\\']) {
                return line.chars().take(512).collect::<String>();
            }
            let trimmed = line.trim_start();
            let source = trimmed.strip_prefix("at ").or_else(|| {
                (trimmed.starts_with(['/', '\\']) || trimmed.as_bytes().get(1) == Some(&b':'))
                    .then_some(trimmed)
            });
            source.map_or_else(
                || "<redacted>".to_owned(),
                |source| {
                    let name = source.rsplit(['/', '\\']).next().unwrap_or("");
                    format!(
                        "  at <redacted>/{}",
                        name.chars().take(512).collect::<String>()
                    )
                },
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}
