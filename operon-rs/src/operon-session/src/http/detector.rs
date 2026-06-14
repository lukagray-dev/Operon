// detector.rs — Streaming tool call detection state machine.
//
// Sits between the SSE assembler output and the UI event channel.
// Buffers streamed text to detect in-progress tool tags, emitting
// prose TextDeltas only for confirmed non-tag content.
//
// Hey friend! Here is a simple explanation of how this works:
// The LLM streams text token by token. A tool call might look like:
// `<read path="foo.txt">` (a bodyless tool)
// or
// `<write path="foo.txt"><<<<some content>>>>` (a tool with a body).
// We don't want to show raw `<read ...>` tags as prose/text to the user.
// Instead, we want to detect them, hide them from the prose text stream,
// and emit special events so the UI can draw nice cards or live previews.
//
// State machine transitions:
//
//   Prose          → Normal text mode. Character-by-character output as TextDelta.
//                    If we see '<', we transition to Suspicious state and start buffering.
//   Suspicious     → We saw a '<'. We buffer characters until we know if it is a tag.
//                    If the next characters form a valid tool name, we go to TagOpen.
//                    If not (e.g. `< ` or `<3`), we flush the buffer as prose and return to Prose.
//   TagOpen        → We matched `<toolname` and are now buffering its attributes until we see `>`.
//                    If we see an unexpected character, we flush buffer as prose and return to Prose.
//   AttrComplete   → An intermediate state when closing `>` is seen. (Not returned from step).
//                    For body-less tools, we emit ToolCallDetected and go back to Prose.
//                    For body-based tools, we transition to BodyPending and wait for `<<<<`.
//   BodyPending    → We saw the closing `>` of a body tool (e.g., `<write...>`).
//                    We buffer up to 32 characters waiting to see `<<<<`.
//                    If we see `<<<<`, we emit ToolBodyStarted and transition to BodyStreaming.
//                    If we don't see `<<<<` within 32 chars, we flush the buffer as prose and return to Prose.
//   BodyStreaming  → We are inside the body of the tool call.
//                    Each character is emitted as a ToolBodyDelta event so the UI can show live updates.
//                    We check the end of the streaming buffer for the closing sequence `>>>>`.
//                    When `>>>>` is detected, we strip it from the delta events, emit ToolCallComplete,
//                    and transition back to Prose state.

/// Tools that use a `<<<<`…`>>>>` body block.
const BODY_TOOLS: &[&str] = &[
    "write", "append", "edit", "bash", "grep", "ls", "delete", "ask",
];

/// All known tool names. Text that matches `<unknown>` is treated as prose.
const ALL_TOOLS: &[&str] = &[
    "read", "write", "append", "edit", "bash", "grep", "ls", "delete", "ask",
    "load_tools",
];

/// The internal state of our detector state machine.
#[derive(Debug)]
enum State {
    /// Normal prose text. We emit everything as TextDelta.
    Prose,
    /// We saw `<` and are waiting to see if it starts a valid tool tag.
    Suspicious { buf: String },
    /// We confirmed a valid tool name and are collecting attributes.
    TagOpen { name: String, buf: String },

    /// Waiting for the `<<<<` body start sequence after a body tool tag was closed.
    BodyPending {
        call_id: String,
        name: String,
        attrs: String,
        head_buf: String,
    },
    /// Inside the tool call body. We stream characters until we see `>>>>`.
    BodyStreaming { call_id: String },
}

/// Events emitted by the detector for each pushed text chunk.
#[derive(Debug)]
pub enum DetectorEvent {
    /// Safe prose — emit as TextDelta immediately.
    TextDelta(String),
    /// Tool tag confirmed (no-body tool) — show card now.
    ToolCallDetected {
        call_id: String,
        name: String,
        attrs: String,
    },
    /// Tool tag confirmed (body tool) + `<<<<` seen — show card now.
    ToolBodyStarted {
        call_id: String,
        name: String,
        attrs: String,
    },
    /// One chunk of body content between `<<<<` and `>>>>`.
    ToolBodyDelta {
        call_id: String,
        text: String,
    },
    /// `>>>>` seen — tool call complete.
    ToolCallComplete {
        call_id: String,
    },
}

/// The StreamingTagDetector tracks the state of the SSE text stream
/// and parses tool calls on the fly.
pub struct StreamingTagDetector {
    /// The current state of the parsing machine.
    state: State,
    /// Monotonic index within this stream, used to build streaming call_ids.
    call_index: usize,
    /// The turn index, set at construction, used to build streaming call_ids.
    turn_index: usize,
}

impl StreamingTagDetector {
    /// Creates a new streaming tag detector for a specific conversation turn.
    pub fn new(turn_index: usize) -> Self {
        Self {
            state: State::Prose,
            call_index: 0,
            turn_index,
        }
    }

    /// Push one text chunk from the SSE assembler.
    /// Returns zero or more events for the caller to process in order.
    pub fn push(&mut self, text: &str) -> Vec<DetectorEvent> {
        let mut events = Vec::new();
        // Hey friend! We process the text character by character so we can easily
        // feed it into our state machine step function.
        for ch in text.chars() {
            self.step(ch, &mut events);
        }
        events
    }

    /// Helper to generate a unique streaming tool call identifier.
    /// Format: `stream-{turn_index}-{call_index}`
    fn next_call_id(&mut self) -> String {
        let id = format!("stream-{}-{}", self.turn_index, self.call_index);
        self.call_index += 1;
        id
    }

    /// Run a single step of the state machine for the given character.
    fn step(&mut self, ch: char, events: &mut Vec<DetectorEvent>) {
        // We replace self.state with a sentinel (State::Prose) to allow moving out of it.
        // This is a common Rust idiom when we need to take ownership of fields in the old state.
        let state = std::mem::replace(&mut self.state, State::Prose);

        self.state = match state {
            // ── Prose ──────────────────────────────────────────────────────
            // We are in normal text. If we see a '<', we start buffering
            // in case this is the start of a tool call tag. Otherwise, we
            // emit the character immediately as a TextDelta.
            State::Prose => {
                if ch == '<' {
                    State::Suspicious { buf: String::from('<') }
                } else {
                    events.push(DetectorEvent::TextDelta(ch.to_string()));
                    State::Prose
                }
            }

            // ── Suspicious ─────────────────────────────────────────────────
            // Saw `<`. Buffering until we know if this is a tool tag or prose.
            State::Suspicious { mut buf } => {
                buf.push(ch);

                if ch.is_ascii_alphabetic() && buf == format!("<{}", ch) {
                    // Could be a tag. Transition to TagOpen with name started.
                    State::TagOpen { name: ch.to_string(), buf }
                } else if ch.is_ascii_alphabetic() {
                    // Still building potential tag name.
                    State::TagOpen { name: ch.to_string(), buf }
                } else {
                    // Not a tag — flush buffer as prose and go back to Prose.
                    events.push(DetectorEvent::TextDelta(buf));
                    State::Prose
                }
            }

            // ── TagOpen ────────────────────────────────────────────────────
            // Accumulating the tag name and attrs.
            State::TagOpen { mut name, mut buf } => {
                buf.push(ch);

                if ch == '>' {
                    // Tag header closed. Check if name is a known tool.
                    if !ALL_TOOLS.contains(&name.as_str()) {
                        // Unknown tag — flush as prose.
                        events.push(DetectorEvent::TextDelta(buf));
                        return self.state = State::Prose;
                    }
                    // Extract attrs: buf is `<toolname attrs>`, strip `<name` and `>`.
                    let inner = buf
                        .trim_start_matches('<')
                        .trim_start_matches(name.as_str())
                        .trim_end_matches('>')
                        .trim()
                        .to_string();
                    let attrs = inner;

                    if BODY_TOOLS.contains(&name.as_str()) {
                        // Body tool — wait for `<<<<`.
                        let call_id = self.next_call_id();
                        State::BodyPending { call_id, name, attrs, head_buf: String::new() }
                    } else {
                        // No-body tool — emit card immediately.
                        let call_id = self.next_call_id();
                        events.push(DetectorEvent::ToolCallDetected {
                            call_id,
                            name,
                            attrs,
                        });
                        State::Prose
                    }
                } else if ch.is_ascii_alphanumeric()
                    || ch == '_'
                    || ch == '-'
                    || ch == ' '
                    || ch == '"'
                    || ch == '='
                    || ch == '\\'
                    || ch == '.'
                    || ch == ':'
                    || ch == '/'
                {
                    // Valid tag char — keep accumulating.
                    // If still in name portion (no space yet), extend name.
                    if !buf.contains(' ') && (ch.is_ascii_alphanumeric() || ch == '_') {
                        name.push(ch);
                    }
                    State::TagOpen { name, buf }
                } else {
                    // Unexpected char — flush as prose.
                    events.push(DetectorEvent::TextDelta(buf));
                    State::Prose
                }
            }

            // ── BodyPending ─────────────────────────────────────────────────
            // Saw closing `>` for a body tool. Waiting for `<<<<`.
            State::BodyPending { call_id, name, attrs, mut head_buf } => {
                head_buf.push(ch);

                if head_buf.ends_with("<<<<") {
                    // Body started. Emit ToolBodyStarted and switch to streaming.
                    events.push(DetectorEvent::ToolBodyStarted {
                        call_id: call_id.clone(),
                        name,
                        attrs,
                    });
                    State::BodyStreaming { call_id }
                } else if head_buf.len() > 32 {
                    // Too much content before `<<<<` — not a valid tool body.
                    // Flush the buffered content as prose and reset.
                    events.push(DetectorEvent::TextDelta(head_buf));
                    State::Prose
                } else {
                    State::BodyPending { call_id, name, attrs, head_buf }
                }
            }

            // ── BodyStreaming ───────────────────────────────────────────────
            // Inside a body block. Stream chars as ToolBodyDelta until `>>>>`.
            State::BodyStreaming { call_id } => {
                events.push(DetectorEvent::ToolBodyDelta {
                    call_id: call_id.clone(),
                    text: ch.to_string(),
                });
                // Check if the last 4 ToolBodyDelta events spell `>>>>`.
                // We detect by checking the accumulated suffix. Track it via a small
                // window in the event vec.
                let suffix: String = events
                    .iter()
                    .rev()
                    .take(4)
                    .filter_map(|e| match e {
                        DetectorEvent::ToolBodyDelta { text, .. } => text.chars().next(),
                        _ => None,
                    })
                    .collect::<Vec<char>>()
                    .into_iter()
                    .rev()
                    .collect();

                if suffix == ">>>>" {
                    // Strip the `>>>>` from the last 4 ToolBodyDelta events.
                    let len = events.len();
                    events.truncate(len - 4);
                    events.push(DetectorEvent::ToolCallComplete {
                        call_id: call_id.clone(),
                    });
                    State::Prose
                } else {
                    State::BodyStreaming { call_id }
                }
            }


        };
    }
}
