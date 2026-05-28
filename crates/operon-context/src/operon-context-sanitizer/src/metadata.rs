use std::time::{Duration, SystemTime, UNIX_EPOCH};

use operon_context_normalize_messages::{ContentBlock, ConversationMessage, MessageRole};
use operon_context_snapshot::Role;

pub(crate) fn inject_metadata(
    mut messages: Vec<ConversationMessage>,
    role: Role,
) -> Vec<ConversationMessage> {
    let Some(user_index) = messages
        .iter()
        .rposition(|message| message.role == MessageRole::User)
    else {
        return messages;
    };

    let metadata_prefix = format!("[Time: {} | Role: {}]\n", now_rfc3339_utc(), role.as_str());

    if let Some(first_text_index) = messages[user_index]
        .content
        .iter()
        .position(|block| matches!(block, ContentBlock::Text(_)))
    {
        if let Some(ContentBlock::Text(text)) =
            messages[user_index].content.get_mut(first_text_index)
        {
            let original = std::mem::take(text);
            *text = format!("{metadata_prefix}{original}");
        }
    } else {
        messages[user_index]
            .content
            .insert(0, ContentBlock::Text(metadata_prefix));
    }

    messages
}

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
    fn prepends_metadata_to_last_user_only() {
        let messages = vec![
            ConversationMessage::user(vec![ContentBlock::Text("first".to_string())]),
            ConversationMessage::assistant(vec![ContentBlock::Text("assistant".to_string())]),
            ConversationMessage::user(vec![ContentBlock::Text("second".to_string())]),
        ];

        let output = inject_metadata(messages, Role::Owner);
        let first_user_text = match &output[0].content[0] {
            ContentBlock::Text(text) => text,
            _ => panic!("expected text block"),
        };
        let second_user_text = match &output[2].content[0] {
            ContentBlock::Text(text) => text,
            _ => panic!("expected text block"),
        };

        assert_eq!(first_user_text, "first");
        assert!(second_user_text.contains("| Role: Owner]"));
        assert!(second_user_text.ends_with("second"));
    }

    #[test]
    fn non_user_messages_are_untouched() {
        let assistant_message =
            ConversationMessage::assistant(vec![ContentBlock::Text("assistant".to_string())]);
        let messages = vec![assistant_message.clone()];

        let output = inject_metadata(messages, Role::External);
        assert_eq!(output, vec![assistant_message]);
    }

    #[test]
    fn metadata_format_starts_with_expected_prefix() {
        let messages = vec![ConversationMessage::user(vec![ContentBlock::Text(
            "hello".to_string(),
        )])];

        let output = inject_metadata(messages, Role::Owner);
        let text = match &output[0].content[0] {
            ContentBlock::Text(text) => text,
            _ => panic!("expected text block"),
        };

        assert!(text.starts_with("[Time: "));
        assert!(text.contains(" | Role: Owner]"));
    }
}
