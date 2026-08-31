use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::types::{BootstrapBlock, Role};

/// The full system instructions prompt defining Operon's identity, guidelines,
/// principles, and behaviors. This is prepended to the system message context.
///
/// Hey friend! Tools are provided directly via the provider's API tools array on every turn,
/// so the model has immediate access to all tools without needing an intermediate discovery step.
const OPERON_SYSTEM_PROMPT: &str = "\
You are Operon, an autonomous coding and operations agent. You have access to tools \
for reading, editing, and running code, and for interacting with external systems.\n\n\
Be direct and concise. Do not narrate what you are about to do — just do it, then report \
results.\n\n\
Use tools to gather context instead of guessing. If you can check something by reading a \
file or running a command, do that rather than assuming.\n\n\
When editing code, match the existing style and conventions of the surrounding codebase \
rather than imposing your own. Prefer minimal, targeted changes over broad rewrites unless \
asked to refactor.\n\n\
Do not create documentation, summaries, or other files unless the user asked for them.\n\n\
If a task is ambiguous or you are missing information needed to proceed safely, ask — do \
not guess on anything destructive or hard to reverse.";

/// Fixed agent identity used in snapshot bootstrap blocks.
const AGENT_NAME: &str = "Operon";

/// Builds the bootstrap section for the current turn.
pub(crate) fn assemble_bootstrap(role: Role, session_id: String) -> BootstrapBlock {
    BootstrapBlock {
        agent_name: AGENT_NAME.to_string(),
        timestamp: now_rfc3339_utc(),
        session_id,
        role,
        system_prompt: OPERON_SYSTEM_PROMPT,
    }
}

/// Converts current UTC time into an RFC3339 timestamp without extra deps.
fn now_rfc3339_utc() -> String {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0));

    let total_seconds = duration.as_secs() as i64;
    let seconds_per_day = 86_400_i64;

    let days_since_epoch = total_seconds / seconds_per_day;
    let seconds_of_day = total_seconds % seconds_per_day;

    let (year, month, day) = civil_from_days(days_since_epoch);
    let hour = seconds_of_day / 3_600;
    let minute = (seconds_of_day % 3_600) / 60;
    let second = seconds_of_day % 60;

    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

/// Civil-date conversion from Unix days since 1970-01-01.
///
/// This is Howard Hinnant's well-known algorithm for Gregorian calendar math.
fn civil_from_days(days_since_epoch: i64) -> (i32, u32, u32) {
    let z = days_since_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    let year = y + if m <= 2 { 1 } else { 0 };

    (year as i32, m as u32, d as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamp_is_rfc3339_like() {
        let ts = now_rfc3339_utc();
        assert_eq!(ts.len(), 20);
        assert!(ts.ends_with('Z'));
        assert_eq!(&ts[4..5], "-");
        assert_eq!(&ts[7..8], "-");
        assert_eq!(&ts[10..11], "T");
        assert_eq!(&ts[13..14], ":");
        assert_eq!(&ts[16..17], ":");
    }

    #[test]
    fn bootstrap_has_fixed_agent_name() {
        let block = assemble_bootstrap(Role::Owner, "abc".to_string());
        assert_eq!(block.agent_name, "Operon");
        assert_eq!(block.system_prompt, OPERON_SYSTEM_PROMPT);
    }
}
