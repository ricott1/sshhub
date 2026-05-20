use std::time::{Duration, Instant};

/// Seconds until idle-kick, rounded up so the displayed countdown ticks down
/// to 1 (not 0) before disconnect. Returns `None` outside the warning window
/// or after the deadline has already passed.
///
/// Use this from a lobby's draw loop to drive a "kicking in Ns" hint.
pub fn kick_warning_secs(
    last_input_at: Instant,
    now: Instant,
    kick_after: Duration,
    warning_within: Duration,
) -> Option<u32> {
    let elapsed = now.saturating_duration_since(last_input_at);
    if elapsed >= kick_after {
        return None;
    }
    let remaining = kick_after - elapsed;
    if remaining >= warning_within {
        return None;
    }
    let secs = remaining.as_secs() as u32 + u32::from(remaining.subsec_nanos() > 0);
    Some(secs)
}

/// Standard idle-warning banner text. Games should render this in a small
/// centered, bordered Rect with red-bold styling so the warning looks
/// identical across the suite. A typical render is:
///
/// ```ignore
/// Paragraph::new(idle_warning_text(secs))
///     .centered()
///     .style(Style::new().red().bold())
///     .block(Block::bordered())
/// ```
pub fn idle_warning_text(secs: u32) -> String {
    format!("idle - kicking in {secs}s, press any key")
}
