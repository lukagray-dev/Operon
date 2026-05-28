//! Integration tests for the `operon-context-normalize-reasoning` crate.
//!
//! # What are these tests verifying?
//! We want to ensure that reasoning/thinking payloads from all supported LLM providers
//! (Anthropic, OpenAI, Gemini, DeepSeek, xAI, Ollama, OpenRouter, Groq, Mistral, Cohere)
//! normalize and denormalize correctly. We also verify all the error paths:
//! missing fields, unsupported features, empty arrays, and unknown wire shapes.
//!
//! # Teaching a Newbie:
//! In Rust, integration tests go under the `tests/` directory. Each file here is compiled
//! as its own separate crate, which simulates a real user importing our crate. We use
//! `serde_json::json!` to quickly build JSON structures that mimic what providers return.

use operon_context_normalize_reasoning::{
    denormalize_reasoning, normalize_reasoning, Provider, ReasoningBlock,
    ReasoningNormalizeError,
};
use serde_json::json;

// ─────────────────────────────────────────────────────────────────────────────
// Anthropic Tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_anthropic_normalization_with_signature() {
    // 1. Arrange: Mimic an Anthropic thinking block with both the reasoning text
    //    and the optional signature field present.
    let raw = json!({
        "type": "thinking",
        "thinking": "We need to approach this problem step-by-step.",
        "signature": "opaque_sig_123"
    });

    // 2. Act: Call our normalize function specifying Anthropic as the provider.
    let result = normalize_reasoning(raw, &Provider::Anthropic);

    // 3. Assert: The result should be Ok, returning exactly one block.
    //    The block should capture both the thinking text and the signature.
    assert!(result.is_ok());
    let blocks = result.unwrap();
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].thinking, "We need to approach this problem step-by-step.");
    assert!(blocks[0].has_signature());
    assert_eq!(
        blocks[0].signature.as_ref().unwrap().0,
        "opaque_sig_123"
    );
}

#[test]
fn test_anthropic_normalization_without_signature() {
    // Arrange: Mimic a response where Anthropic omitted the optional signature.
    let raw = json!({
        "type": "thinking",
        "thinking": "No signature here."
    });

    // Act: Normalize it.
    let blocks = normalize_reasoning(raw, &Provider::Anthropic).unwrap();

    // Assert: We should get one block containing the thinking text, but signature is None.
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].thinking, "No signature here.");
    assert!(!blocks[0].has_signature());
}

#[test]
fn test_anthropic_normalization_missing_field() {
    // Arrange: Pass a JSON object that lacks the mandatory "thinking" field.
    let raw = json!({
        "type": "thinking",
        "signature": "some_signature"
    });

    // Act: Try to normalize it.
    let result = normalize_reasoning(raw, &Provider::Anthropic);

    // Assert: It should fail with a MissingField error pointing to "thinking".
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(
        err,
        ReasoningNormalizeError::MissingField {
            field: "thinking",
            provider: "Anthropic"
        }
    ));
}

#[test]
fn test_anthropic_denormalization() {
    // Arrange: Create a canonical reasoning block with a signature.
    let blocks = vec![ReasoningBlock::with_signature("Thinking text...", "sig_abc")];

    // Act: Denormalize it to Anthropic wire format.
    let wire = denormalize_reasoning(&blocks, &Provider::Anthropic).unwrap();

    // Assert: It must map to a JSON array containing a single "type":"thinking" object
    // with "thinking" and "signature" fields.
    assert_eq!(
        wire,
        json!([
            {
                "type": "thinking",
                "thinking": "Thinking text...",
                "signature": "sig_abc"
            }
        ])
    );
}

#[test]
fn test_anthropic_denormalization_omits_signature_key() {
    // Arrange: Create a block with NO signature.
    let blocks = vec![ReasoningBlock::new("Thinking text...")];

    // Act: Denormalize it.
    let wire = denormalize_reasoning(&blocks, &Provider::Anthropic).unwrap();

    // Assert: The "signature" key must be entirely absent from the generated JSON,
    // rather than set to null.
    let expected = json!([
        {
            "type": "thinking",
            "thinking": "Thinking text..."
        }
    ]);
    assert_eq!(wire, expected);
}

// ─────────────────────────────────────────────────────────────────────────────
// OpenAI Tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_openai_normalization_success() {
    // Arrange: OpenAI reasoning_summary array containing multiple segments.
    let raw = json!([
        { "type": "summary_text", "text": "Step 1: Parse the user input." },
        { "type": "summary_text", "text": "Step 2: Generate response." }
    ]);

    // Act: Normalize.
    let blocks = normalize_reasoning(raw, &Provider::OpenAI).unwrap();

    // Assert: We should get back two canonical blocks, both without signatures.
    assert_eq!(blocks.len(), 2);
    assert_eq!(blocks[0].thinking, "Step 1: Parse the user input.");
    assert!(!blocks[0].has_signature());
    assert_eq!(blocks[1].thinking, "Step 2: Generate response.");
    assert!(!blocks[1].has_signature());
}

#[test]
fn test_openai_normalization_empty_array() {
    // Arrange: An empty array means no reasoning was performed.
    let raw = json!([]);

    // Act: Normalize.
    let result = normalize_reasoning(raw, &Provider::OpenAI);

    // Assert: Should return an EmptyReasoningSummary error.
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(
        err,
        ReasoningNormalizeError::EmptyReasoningSummary { provider: "OpenAI" }
    ));
}

#[test]
fn test_openai_normalization_missing_fields() {
    // Case A: Missing the top-level array (e.g. caller passed wrong field or null)
    let err_non_array = normalize_reasoning(json!({}), &Provider::OpenAI).unwrap_err();
    assert!(matches!(
        err_non_array,
        ReasoningNormalizeError::MissingField {
            field: "reasoning_summary",
            provider: "OpenAI"
        }
    ));

    // Case B: Element in array is missing the "text" field
    let raw_bad_elem = json!([
        { "type": "summary_text" }
    ]);
    let err_missing_text = normalize_reasoning(raw_bad_elem, &Provider::OpenAI).unwrap_err();
    assert!(matches!(
        err_missing_text,
        ReasoningNormalizeError::MissingField {
            field: "text",
            provider: "OpenAI"
        }
    ));
}

#[test]
fn test_openai_denormalization() {
    // Arrange: Canonical blocks representing two parts of a chain of thought.
    let blocks = vec![
        ReasoningBlock::new("First part."),
        ReasoningBlock::new("Second part."),
    ];

    // Act: Denormalize.
    let wire = denormalize_reasoning(&blocks, &Provider::OpenAI).unwrap();

    // Assert: It should output an array of summary_text objects.
    let expected = json!([
        { "type": "summary_text", "text": "First part." },
        { "type": "summary_text", "text": "Second part." }
    ]);
    assert_eq!(wire, expected);
}

// ─────────────────────────────────────────────────────────────────────────────
// Gemini Tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_gemini_normalization_with_signature() {
    // Arrange: Gemini thought part containing text and thoughtSignature.
    let raw = json!({
        "text": "Thinking about Gemini models...",
        "thought": true,
        "thoughtSignature": "gemini_sig_xyz"
    });

    // Act: Normalize.
    let blocks = normalize_reasoning(raw, &Provider::Gemini).unwrap();

    // Assert: Return exactly one block containing the signature.
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].thinking, "Thinking about Gemini models...");
    assert!(blocks[0].has_signature());
    assert_eq!(
        blocks[0].signature.as_ref().unwrap().0,
        "gemini_sig_xyz"
    );
}

#[test]
fn test_gemini_normalization_without_signature() {
    // Arrange: thoughtSignature is optional (e.g. Gemini 2.5).
    let raw = json!({
        "text": "Gemini 2.5 thinking.",
        "thought": true
    });

    // Act: Normalize.
    let blocks = normalize_reasoning(raw, &Provider::Gemini).unwrap();

    // Assert: One block, no signature.
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].thinking, "Gemini 2.5 thinking.");
    assert!(!blocks[0].has_signature());
}

#[test]
fn test_gemini_normalization_missing_text() {
    // Arrange: A thought part missing the required "text" field.
    let raw = json!({
        "thought": true
    });

    // Act: Normalize.
    let result = normalize_reasoning(raw, &Provider::Gemini);

    // Assert: Should return MissingField error for "text".
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(
        err,
        ReasoningNormalizeError::MissingField {
            field: "text",
            provider: "Gemini"
        }
    ));
}

#[test]
fn test_gemini_denormalization() {
    // Arrange: A block with a signature.
    let blocks = vec![ReasoningBlock::with_signature("Hello Gemini", "signature_123")];

    // Act: Denormalize.
    let wire = denormalize_reasoning(&blocks, &Provider::Gemini).unwrap();

    // Assert: Output should be a JSON array of thought part objects, containing thoughtSignature.
    let expected = json!([
        {
            "text": "Hello Gemini",
            "thought": true,
            "thoughtSignature": "signature_123"
        }
    ]);
    assert_eq!(wire, expected);
}

#[test]
fn test_gemini_denormalization_omits_signature_key() {
    // Arrange: Block with NO signature.
    let blocks = vec![ReasoningBlock::new("Hello Gemini")];

    // Act: Denormalize.
    let wire = denormalize_reasoning(&blocks, &Provider::Gemini).unwrap();

    // Assert: The "thoughtSignature" key must be entirely omitted.
    let expected = json!([
        {
            "text": "Hello Gemini",
            "thought": true
        }
    ]);
    assert_eq!(wire, expected);
}

// ─────────────────────────────────────────────────────────────────────────────
// DeepSeek & xAI & Ollama Tests (Plain String Payload formats)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_deepseek_and_xai_and_ollama_normalization() {
    let inputs = vec![
        (Provider::DeepSeek, "reasoning_content"),
        (Provider::XAI, "reasoning_content"),
        (Provider::Ollama, "thinking"),
    ];

    for (provider, field_name) in inputs {
        // Arrange: A plain string payload containing the reasoning.
        let raw = json!("Thinking through the problem.");

        // Act: Normalize.
        let blocks = normalize_reasoning(raw.clone(), &provider).unwrap();

        // Assert: Return exactly one block with no signature.
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].thinking, "Thinking through the problem.");
        assert!(!blocks[0].has_signature());

        // Test Error: Passing a non-string payload should result in MissingField error.
        let bad_raw = json!(12345);
        let err = normalize_reasoning(bad_raw, &provider).unwrap_err();
        assert!(matches!(
            err,
            ReasoningNormalizeError::MissingField {
                field,
                ..
            } if field == field_name
        ));
    }
}

#[test]
fn test_deepseek_and_xai_and_ollama_denormalization() {
    let providers = vec![Provider::DeepSeek, Provider::XAI, Provider::Ollama];
    let blocks = vec![
        ReasoningBlock::new("Part 1."),
        ReasoningBlock::new("Part 2."),
    ];

    for provider in providers {
        // Act: Denormalize.
        let wire = denormalize_reasoning(&blocks, &provider).unwrap();

        // Assert: A single string joining the reasoning of all blocks with double newlines.
        assert_eq!(wire, json!("Part 1.\n\nPart 2."));
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// OpenRouter Tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_openrouter_shape_detection_string() {
    // Arrange: OpenRouter proxies a DeepSeek response (which is a plain string).
    let raw = json!("Proxied DeepSeek string.");

    // Act: Normalize.
    let blocks = normalize_reasoning(raw, &Provider::OpenRouter).unwrap();

    // Assert: Successfully parsed into a single block.
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].thinking, "Proxied DeepSeek string.");
    assert!(!blocks[0].has_signature());
}

#[test]
fn test_openrouter_shape_detection_array() {
    // Arrange: OpenRouter proxies an OpenAI response (an array of summary blocks).
    let raw = json!([
        { "type": "summary_text", "text": "Proxied step." }
    ]);

    // Act: Normalize.
    let blocks = normalize_reasoning(raw, &Provider::OpenRouter).unwrap();

    // Assert: Successfully parsed.
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].thinking, "Proxied step.");
}

#[test]
fn test_openrouter_shape_detection_anthropic_object() {
    // Arrange: OpenRouter proxies an Anthropic response (type: thinking).
    let raw = json!({
        "type": "thinking",
        "thinking": "Claude thoughts.",
        "signature": "claude_sig"
    });

    // Act: Normalize.
    let blocks = normalize_reasoning(raw, &Provider::OpenRouter).unwrap();

    // Assert: Successfully parsed with signature.
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].thinking, "Claude thoughts.");
    assert_eq!(blocks[0].signature.as_ref().unwrap().0, "claude_sig");
}

#[test]
fn test_openrouter_shape_detection_gemini_object() {
    // Arrange: OpenRouter proxies a Gemini response (thought: true).
    let raw = json!({
        "text": "Gemini thoughts.",
        "thought": true,
        "thoughtSignature": "gemini_sig"
    });

    // Act: Normalize.
    let blocks = normalize_reasoning(raw, &Provider::OpenRouter).unwrap();

    // Assert: Successfully parsed with signature.
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].thinking, "Gemini thoughts.");
    assert_eq!(blocks[0].signature.as_ref().unwrap().0, "gemini_sig");
}

#[test]
fn test_openrouter_shape_detection_unknown() {
    // Arrange: Pass an object with unknown fields.
    let raw = json!({
        "unrelated_field": "values"
    });

    // Act: Try to normalize.
    let err = normalize_reasoning(raw, &Provider::OpenRouter).unwrap_err();

    // Assert: Returns UnknownShape.
    assert!(matches!(
        err,
        ReasoningNormalizeError::UnknownShape {
            provider: "OpenRouter",
            ..
        }
    ));
}

#[test]
fn test_openrouter_denormalization() {
    // Arrange: Canonical blocks.
    let blocks = vec![ReasoningBlock::new("OpenRouter output.")];

    // Act: Denormalize.
    let wire = denormalize_reasoning(&blocks, &Provider::OpenRouter).unwrap();

    // Assert: Must format as an OpenAI-compatible reasoning_summary array.
    let expected = json!([
        { "type": "summary_text", "text": "OpenRouter output." }
    ]);
    assert_eq!(wire, expected);
}

// ─────────────────────────────────────────────────────────────────────────────
// Unsupported Providers Tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_unsupported_providers() {
    let unsupported = vec![Provider::Groq, Provider::Mistral, Provider::Cohere];

    for provider in unsupported {
        let name = match provider {
            Provider::Groq => "Groq",
            Provider::Mistral => "Mistral",
            Provider::Cohere => "Cohere",
            _ => unreachable!(),
        };

        // Act & Assert for Normalize
        let norm_err = normalize_reasoning(json!("Thinking..."), &provider).unwrap_err();
        assert!(matches!(
            norm_err,
            ReasoningNormalizeError::NotSupported { provider: p } if p == name
        ));

        // Act & Assert for Denormalize
        let blocks = vec![ReasoningBlock::new("Thinking...")];
        let denorm_err = denormalize_reasoning(&blocks, &provider).unwrap_err();
        assert!(matches!(
            denorm_err,
            ReasoningNormalizeError::NotSupported { provider: p } if p == name
        ));
    }
}
