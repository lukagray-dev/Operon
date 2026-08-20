#!/usr/bin/env python3
"""
Update Model Catalog Dataset for Operon
========================================

This script downloads and synchronizes model context window specifications and
max output token limits from upstream AI provider registries (LiteLLM and OpenRouter).

It generates the embedded static dataset located at:
  operon-rs/src/operon-providers/src/data/models.json

Usage:
  python scripts/update-model-catalog.py
"""

import json
import os
import sys
import urllib.request


def fetch_json(url: str, description: str) -> dict:
    """
    Helper function to download and parse JSON from a public URL.
    Includes standard HTTP headers to avoid being blocked.
    """
    print(f"[*] Fetching {description} from {url}...")
    headers = {
        "User-Agent": "Operon-Model-Catalog-Sync/1.0",
        "Accept": "application/json",
    }
    request = urllib.request.Request(url, headers=headers)
    try:
        with urllib.request.urlopen(request, timeout=30) as response:
            raw_data = response.read().decode("utf-8")
            return json.loads(raw_data)
    except Exception as e:
        print(f"[!] Error fetching {description}: {e}", file=sys.stderr)
        return {}


def main():
    # Destination path in the repository
    repo_root = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
    target_file = os.path.join(
        repo_root,
        "operon-rs",
        "src",
        "operon-providers",
        "src",
        "data",
        "models.json"
    )

    models_map = {}

    # 1. Ingest LiteLLM model registry (~4,000+ entries across all cloud providers)
    # LiteLLM tracks max_input_tokens, max_output_tokens, pricing, and capabilities.
    litellm_url = "https://raw.githubusercontent.com/BerriAI/litellm/main/model_prices_and_context_window.json"
    litellm_data = fetch_json(litellm_url, "LiteLLM model registry")

    for model_id, info in litellm_data.items():
        # Skip doc placeholder sample specs
        if not isinstance(info, dict) or model_id == "sample_spec":
            continue

        # Extract context window tokens
        ctx = info.get("max_input_tokens") or info.get("max_tokens")
        max_out = info.get("max_output_tokens") or 8192

        if ctx and isinstance(ctx, int) and ctx > 0:
            entry = {
                "context_window": int(ctx),
                "max_output_tokens": int(max_out) if isinstance(max_out, int) and max_out > 0 else 8192,
            }
            # Key with lowercase identifier for case-insensitive O(1) matching
            canonical_key = model_id.strip().lower()
            models_map[canonical_key] = entry

            # Also register the short bare name without vendor prefix (e.g. "openai/gpt-4o" -> "gpt-4o")
            if "/" in canonical_key:
                _, short_name = canonical_key.split("/", 1)
                short_name = short_name.strip()
                if short_name and short_name not in models_map:
                    models_map[short_name] = entry

    litellm_count = len(models_map)
    print(f"[+] Loaded {litellm_count} model specs from LiteLLM.")

    # 2. Ingest OpenRouter live model catalog (~300+ frontier and open-weight models)
    # OpenRouter contains real-time context lengths for new releases (e.g. Claude 3.7, DeepSeek V4, Gemini 3.7).
    openrouter_url = "https://openrouter.ai/api/v1/models"
    openrouter_data = fetch_json(openrouter_url, "OpenRouter model catalog")

    openrouter_items = openrouter_data.get("data", [])
    for item in openrouter_items:
        model_id = item.get("id")
        if not model_id or not isinstance(model_id, str):
            continue

        ctx = item.get("context_length") or 128000
        top_provider = item.get("top_provider") or {}
        max_out = top_provider.get("max_completion_tokens") or 8192

        entry = {
            "context_window": int(ctx),
            "max_output_tokens": int(max_out) if isinstance(max_out, int) and max_out > 0 else 8192,
        }

        canonical_key = model_id.strip().lower()
        models_map[canonical_key] = entry

        # Also register bare name without vendor namespace
        if "/" in canonical_key:
            _, short_name = canonical_key.split("/", 1)
            short_name = short_name.strip()
            if short_name and short_name not in models_map:
                models_map[short_name] = entry

    total_count = len(models_map)
    print(f"[+] Total indexed models across all providers: {total_count}")

    # Ensure parent directory exists
    os.makedirs(os.path.dirname(target_file), exist_ok=True)

    # Write sorted, formatted JSON file
    with open(target_file, "w", encoding="utf-8") as f:
        json.dump(models_map, f, indent=2, sort_keys=True)

    print(f"[OK] Successfully wrote {total_count} models to:")
    print(f"    {target_file}")


if __name__ == "__main__":
    main()
