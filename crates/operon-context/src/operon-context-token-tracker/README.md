# operon-context-token-tracker

Token estimation and budget management for LLM context window control.

Part of the [Operon](https://github.com/lukagray-dev/operon) project but independently usable by any Rust developer building LLM applications.

---

## Overview

Every LLM has a hard context-window limit. Exceeding it causes truncation or rejection. This crate provides a **three-tier token counting strategy** that lets you estimate, track, and budget token usage across an entire agent session — before and after each API call.

| Tier      | When       | Accuracy | Dependency       |
|-----------|------------|----------|------------------|
| Exact     | Post-call  | Perfect  | Provider API     |
| BPE       | Pre-call   | ~99%     | `bpe` feature    |
| Heuristic | Pre-call   | ~85-90%  | None             |

---

## Architecture

```
┌─────────────────────────────────────────┐
│            Your LLM Agent               │
├────────────┬────────────┬───────────────┤
│  Pre-call  │  API call  │  Post-call    │
│            │            │               │
│ TokenEsti- │    LLM     │ TokenRecorder │
│ mator      │  Provider  │ .record()     │
│ .estimate()│            │               │
│            │            │ SessionToken  │
│ SessionTo- │            │ State         │
│ kenState   │            │ .record_turn()│
│ .apply_    │            │               │
│  estimate()│            │               │
├────────────┴────────────┴───────────────┤
│           TokenBudget                   │
│         .should_compact()               │
└─────────────────────────────────────────┘
```

### Tier 1 — Exact (post-call)

Record the actual token counts from the provider API response. Every major provider includes this in their response body:

- Anthropic: `usage.input_tokens`, `usage.output_tokens`
- OpenAI: `usage.prompt_tokens`, `usage.completion_tokens`

This is ground truth. Always use it to correct session state after each turn.

### Tier 2 — BPE (pre-call, `bpe` feature)

Uses the `tiktoken` crate with `cl100k_base` encoding — the industry standard for OpenAI-family models. Accurate to ~99% for GPT-3.5/4, DeepSeek, and Nvidia NIM. Enable via the `bpe` Cargo feature.

### Tier 3 — Heuristic (pre-call, always available)

Zero-dependency content-aware estimation:

- **Code** (`>= 15%` code characters): `byte_len / 3`
- **Prose**: `byte_len / 4`

Intentionally overshoots slightly (conservative bias) so compaction triggers before the hard limit, not after.

---

## Quick Start

```toml
# Cargo.toml
[dependencies]
operon-context-token-tracker = "0.1"

# Optional: enable BPE tier
# operon-context-token-tracker = { version = "0.1", features = ["bpe"] }
```

```rust
use operon_context_token_tracker::{
    TokenEstimator, TokenBudget, SessionTokenState, UsageRecord, TokenRecorder,
};

// Pre-call: estimate before sending to the LLM
let (count, tier) = TokenEstimator::estimate("Hello, world!");
println!("~{count} tokens (via {tier:?})");

// Budget: check if compaction is needed
let budget = TokenBudget::with_window(200_000)?;
if budget.should_compact(current_tokens) {
    // trigger compaction before next call
}

// Post-call: record exact usage from the API response
let mut session = SessionTokenState::new();
let record = UsageRecord {
    input_tokens:       1_024,
    output_tokens:      256,
    cache_read_tokens:  None,
    cache_write_tokens: None,
    model:    "claude-sonnet-4-5".into(),
    provider: "anthropic".into(),
};
session.record_turn(&record);

// Check if context window needs compaction
if budget.should_compact(session.current_context_tokens) {
    // run compaction, then session.reset()
}
```

---

## API Reference

### `TokenEstimator`

Stateless. All methods are associated functions.

| Method | Description |
|---|---|
| `estimate(text)` | Estimate tokens for a single string. Returns `(count, tier)`. |
| `estimate_messages(msgs)` | Estimate for a slice of messages, adding 4 tokens/message overhead. |
| `is_code_like(text)` | Returns `true` if `>= 15%` of chars are code indicators. |

### `TokenBudget`

Immutable configuration for one model session.

| Method | Description |
|---|---|
| `with_window(n)` | Construct with 90% compaction threshold. |
| `new(n, threshold)` | Construct with custom threshold in `(0.0, 1.0]`. |
| `should_compact(tokens)` | `true` when at or past the compaction limit. |
| `remaining(tokens)` | Tokens left before the compaction limit. |
| `utilization(tokens)` | Fraction consumed `[0.0, 1.0]`. |
| `compaction_limit()` | Absolute token count at which compaction triggers. |

### `SessionTokenState`

Mutable cross-turn state. Persist this between agent turns.

| Method | Description |
|---|---|
| `record_turn(&record)` | Update from a post-call `UsageRecord` (Tier 1). |
| `apply_estimate(n, tier)` | Update `current_context_tokens` from a pre-call estimate. |
| `reset()` | Zero all state for a new session or post-compaction. |
| `total_tokens()` | Sum of all input + output tokens this session. |

### `TokenRecorder`

Accumulates `UsageRecord`s. History is newest-first.

| Method | Description |
|---|---|
| `record(usage)` | Append a usage record. |
| `last()` | Most recent record. |
| `history()` | All records, newest first. |
| `total_input()` / `total_output()` / `total_tokens()` | Aggregate stats. |
| `clear()` | Discard all records. |

### `UsageRecord`

```rust
pub struct UsageRecord {
    pub input_tokens:        usize,
    pub output_tokens:       usize,
    pub cache_read_tokens:   Option<usize>,  // Anthropic prompt caching
    pub cache_write_tokens:  Option<usize>,  // Anthropic prompt caching
    pub model:               String,
    pub provider:            String,
}
```

---

## Features

| Feature | Default | Description |
|---|---|---|
| `bpe` | off | Enable BPE tokenisation via `tiktoken` (`cl100k_base`). |

The `bpe` feature is strictly additive — disabling it never breaks compilation or changes the public API. When off, `EstimationTier::Bpe` is never returned.

---

## Correctness Guarantees

- No `unwrap()` or `expect()` in library code.
- All counter arithmetic uses `saturating_add` (counters) or `checked_add` + `Err(Overflow)` where overflow would corrupt a budget decision.
- `TokenBudget::new()` validates both parameters and rejects NaN/infinity thresholds.
- `cargo test` and `cargo clippy -- -D warnings` pass clean.

---

## License

AGPL-3.0. See [LICENSE](../../../../LICENSE).
