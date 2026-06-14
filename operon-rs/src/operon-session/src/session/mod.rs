// mod.rs — Declarations and re-exports for the session submodule.
//
// Hey friend! This file declares all our submodules (ask, commands, compaction,
// dispatch, events, init, policy) and re-exports everything required by the runner
// so it can access them conveniently.

pub mod ask;
pub mod commands;
pub mod compaction;
pub mod dispatch;
pub mod events;
pub mod init;
pub mod policy;

pub use ask::{handle_ask_intercept, AskInterceptOutcome};
pub use dispatch::{handle_tool_call, DispatchOutcome};
pub use events::{
    build_assistant_message, context_usage_event, extract_usage_record,
    tool_result_content_json,
};
pub use policy::{opaque_permission_denied_result, policy_path_for_call};
