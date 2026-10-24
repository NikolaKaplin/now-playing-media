pub(crate) fn validate_control(
    command: &str,
    session_id: &str,
    position: Option<f64>,
) -> Result<(), &'static str> {
    if !matches!(
        command,
        "play" | "pause" | "toggle" | "next" | "previous" | "seek"
    ) {
        return Err("Unknown playback command.");
    }
    if session_id.len() > 4096 || session_id.contains('\0') {
        return Err("Invalid media session id.");
    }
    if command == "seek"
        && !position.is_some_and(|value| {
            value.is_finite()
                && (0.0..=9_007_199_254_740_991.0).contains(&value)
                && value.fract() == 0.0
        })
    {
        return Err("Seek position must be a non-negative integer number of milliseconds.");
    }
    Ok(())
}
