<!-- ============================================================================== -->
<!-- Operon - Pull Request Template                                                 -->
<!-- ============================================================================== -->
<!-- Thank you for contributing to Operon!                                          -->
<!-- Please make sure you have read CONTRIBUTING.md before submitting your PR.      -->
<!-- NOTE: Pull Requests must target the `dev` branch, NOT `main`.                  -->
<!-- ============================================================================== -->

## Description

<!-- Provide a clear, detailed summary of the changes made and the motivation behind them. -->
<!-- Explain the problem being solved or the feature being introduced. -->

## Related Issue(s)

<!-- Link any related issues using GitHub keywords (e.g. Closes #123, Fixes #456, Relates to #789). -->

- Closes #

## Type of Change

<!-- Please check all options that apply to this change: -->

- [ ] **Bug fix** (non-breaking change which fixes an issue)
- [ ] **New feature** (non-breaking change which adds functionality)
- [ ] **Breaking change** (fix or feature that causes existing APIs, configs, or behaviors to change)
- [ ] **Refactoring / Cleanup** (code structure improvement without behavioral changes)
- [ ] **Documentation update** (updating guides, docstrings, or architecture diagrams)
- [ ] **Performance optimization** (improving speed, memory footprint, or token efficiency)
- [ ] **Build / CI / Tooling** (updating scripts, workflows, or dependencies)

## Subsystem & Crates Affected

<!-- Select the crates or directories touched by this PR: -->

- [ ] `gui/` (Tauri v2 Desktop App & TypeScript Frontend)
- [ ] `tui/` (Ratatui Terminal Client)
- [ ] `vscode/` (VS Code Extension & Rust Bridge)
- [ ] `operon-rs` Core Engine (`operon-session`, `operon-context`, `operon-providers`)
- [ ] `operon-policy` (Permission Engine & Role Access Control)
- [ ] `operon-tools` (Tool implementations: fs, shell, web, memory, etc.)
- [ ] `operon-channels` (Remote integrations: WhatsApp, Telegram)
- [ ] `operon-config` / `operon-terminal` / `operon-events`
- [ ] `scripts/` / `docs/` / `assets/`

## Security & Permission Model Review

<!-- Operon enforces a strict boundary between Owner and External roles. -->
<!-- If this PR adds or modifies tools, file access, shell execution, or remote channel handling: -->
<!-- 1. How does this change respect the Owner vs External role boundary? -->
<!-- 2. Are directory scopes properly validated and enforced? -->
<!-- 3. Is the permission mode correctly handled (Allow, Ask, Deny)? -->

- [ ] This change does not alter security-sensitive boundaries or tool execution.
- [ ] **OR** This change touches tool/permission logic, and I have documented the security considerations below:
  <!-- Document security and permission considerations here if applicable -->

## UI & Design Standards (Frontend / GUI / TUI PRs)

<!-- If this PR modifies user interfaces, please verify the following: -->

- [ ] **No Emojis in UI**: Used professional-grade vector SVGs, PNGs, or drawables instead of emojis.
- [ ] **No Placeholders**: No broken links, dummy text, or incomplete screens.
- [ ] **Consistent Styling**: Complies with the Operon typography, dark-mode color tokens, and design system.
- [ ] **Visual Proof**: Attached screenshots or a screen recording / GIF demonstrating the UI changes.

<!-- If applicable, paste screenshots / GIFs below: -->

## Code Quality & Testing Checklist

<!-- Please ensure all of the following checks are complete before requesting a review: -->

- [ ] **Target Branch**: This PR targets the `dev` branch (not `main`).
- [ ] **Tests Pass**: `cargo test --workspace` passes without errors or unhandled panics.
- [ ] **Frontend Builds**: `npm run build` succeeds cleanly (if modifying `gui/` or `vscode/extension`).
- [ ] **Inline Comments**: Complex logic and architectural choices are thoroughly commented (explaining the *why*, as if explaining to a new teammate).
- [ ] **Semantic Commits**: Commits follow the semantic format (e.g., `feat(gui): ...`, `fix(policy): ...`, `docs: ...`).
- [ ] **Documentation**: Updated relevant documentation in `docs/` or `README.md` if public APIs, configs, or behaviors changed.
