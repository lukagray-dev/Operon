# Typography & Font Usage

This document defines the strict font hierarchy and typography guidelines across Operon GUI.

---

## 1. Font Families & Roles

All fonts are bundled locally in `gui/src/assets/fonts/` (no external CDNs or network requests).

| Font Family | Location | Strict Usage Scope |
|---|---|---|
| **Open Sans** | `gui/src/assets/fonts/OpenSans/` | **Entire UI** — Navigation, titlebar, sidebars, buttons, labels, settings, dropdowns, and user prompt text. |
| **Kode Mono** | `gui/src/assets/fonts/KodeMono/` | **All Monospace Text** — Code blocks, inline code, terminal outputs, keyboard shortcuts (`Ctrl+N`), and Git diffs. |
| **Literata** | `gui/src/assets/fonts/Literata/` | **Assistant Responses Only** — AI assistant prose, explanation text, and markdown paragraphs for optimal readability. |

---

## 2. CSS Configuration & Tokens

Declared in `gui/src/css/shared/tokens.css`:

```css
:root {
  /* Default UI Font */
  --font-family: 'Open Sans', -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;

  /* Monospace Font */
  --mono-font-family: 'Kode Mono', monospace;

  /* Assistant Message Font */
  --assistant-font-family: 'Literata', Georgia, serif;
}
```

---

## 3. Styling Rules

- **General UI**: Inherits `--font-family` by default.
- **Code & Shortcuts**: Apply `font-family: var(--mono-font-family)`.
- **Assistant Messages**: Apply `font-family: var(--assistant-font-family)` to assistant markdown content containers (`.message-card.assistant .content`).
