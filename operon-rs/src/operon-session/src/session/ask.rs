// ask.rs — Interception logic for the special "ask" tool call.
//
// Hey friend! The "ask" tool is a unique tool: instead of executing a system command
// or file operation, it prompts the user with a question and suspends the agent loop
// until the user responds via the command channel (or cancels).

use std::collections::VecDeque;
use tokio::sync::mpsc;

use operon_context::{ToolCall, ToolContent, ToolResult};
use operon_events::{SessionCommand, SessionEvent};
use operon_tools_ask::AskArgs;

use super::commands::wait_for_relevant;
use super::events::tool_result_content_json;

/// Outcome of the ask intercept function.
pub enum AskInterceptOutcome {
    /// The user responded (or the call failed during parse/validation) and we have a result.
    Responded(ToolResult),
    /// The session loop was cancelled while waiting for a response.
    Cancelled,
}

/// Extracted ask tool interceptor. Suspends the loop to await user input.
pub async fn handle_ask_intercept(
    call: &ToolCall,
    event_tx: &mpsc::Sender<SessionEvent>,
    cmd_rx: &mut mpsc::Receiver<SessionCommand>,
    pending_commands: &mut VecDeque<SessionCommand>,
) -> AskInterceptOutcome {
    let ask_id = call.id.0.clone();

    // Hey friend! We parse and validate the arguments before suspending the loop.
    // If parsing fails (for example, if a required body key is missing), we return
    // an error ToolResult immediately without suspending.
    let ask_result = match AskArgs::parse(&call.arguments) {
        Err(reason) => {
            let result = ToolResult {
                call_id: call.id.clone(),
                name: "ask".to_string(),
                content: ToolContent::Text(reason.to_string()),
                is_error: true,
                read_paths: None,
            };
            let _ = event_tx
                .send(SessionEvent::ToolCallResult {
                    call_id: ask_id.clone(),
                    name: "ask".to_string(),
                    is_error: true,
                    content_json: tool_result_content_json(&result),
                })
                .await;
            return AskInterceptOutcome::Responded(result);
        }
        Ok(args) => args,
    };

    // Emit AskQuestion event. The frontend UI will receive this and render
    // the multiple-choice question widget to the user.
    // Build the options vec from the three individual body-key fields.
    let options_vec = vec![
        ask_result.option1.clone(),
        ask_result.option2.clone(),
        ask_result.option3.clone(),
    ];
    let _ = event_tx
        .send(SessionEvent::AskQuestion {
            id: ask_id.clone(),
            question: ask_result.question.clone(),
            options: options_vec.clone(),
        })
        .await;

    // Suspend the loop and block here until we receive the answer command or a cancel command.
    let answer = loop {
        match wait_for_relevant(cmd_rx, pending_commands, Some(&ask_id)).await {
            SessionCommand::AskResponse { id, answer } if id == ask_id => {
                break answer;
            }
            SessionCommand::Cancel => {
                return AskInterceptOutcome::Cancelled;
            }
            _ => continue,
        }
    };

    // Hey friend! We build a plain-text ToolResult from the user's answer.
    // Format: "Question: {question}\nUser chose: option N. {content}" for numbered
    // options, or "Question: {question}\nUser wrote: {text}" for free-text input.
    let answer_line = options_vec
        .iter()
        .enumerate()
        .find(|(_, opt)| *opt == &answer)
        .map(|(i, opt)| format!("User chose: option {}. {}", i + 1, opt))
        .unwrap_or_else(|| format!("User wrote: {}", answer));
    let output_text = format!(
        "Question: {}\n{}",
        ask_result.question, answer_line
    );
    let content = ToolContent::Text(output_text);
    let result = ToolResult {
        call_id: call.id.clone(),
        name: "ask".to_string(),
        content: content.clone(),
        is_error: false,
        read_paths: None,
    };
    let _ = event_tx
        .send(SessionEvent::ToolCallResult {
            call_id: ask_id.clone(),
            name: "ask".to_string(),
            is_error: false,
            content_json: tool_result_content_json(&result),
        })
        .await;

    AskInterceptOutcome::Responded(result)
}
