# Contributing to Operon

Welcome! We are thrilled that you want to help make **Operon** the best autonomous AI agent for everyone. 

Whether you are fixing a bug, adding a new model provider, implementing a system tool, or polishing the user interface, your contributions are highly appreciated. 

This document serves as a comprehensive guide to help you set up your development environment, understand our codebase structure, write code that matches our quality standards, and submit your changes.

---

## Table of Contents
1. [Code of Conduct](#code-of-conduct)
2. [Codebase Architecture](#codebase-architecture)
3. [Prerequisites & Development Setup](#prerequisites--development-setup)
4. [Development Workflows](#development-workflows)
5. [Coding & Design Standards](#coding--design-standards)
6. [Submitting Your Contribution](#submitting-your-contribution)

---

## Code of Conduct

By participating in this project, you agree to abide by our [Code of Conduct](./CODE_OF_CONDUCT.md). Please read it to understand the expectations we have for all community members.

---

## Codebase Architecture

Operon is structured as a Rust-centric workspace with decoupled frontend clients. Understanding where components live will help you locate the files you need to modify:

*   **[`operon-rs/`](file:///d:/Project%20Operon/Operon/operon-rs)**: The core engine. It manages configuration, provider models, session runners, token estimation, and diagnostic tools.
*   **[`gui/`](file:///d:/Project%20Operon/Operon/gui)**: The desktop GUI built on **Tauri v2**.
    *   `gui/src`: The frontend UI assets (HTML, vanilla CSS, and JavaScript).
    *   `gui/src-tauri`: The Tauri rust backend wrapper, handling window commands, filesystem checks, and communication with the core `operon-rs` engine.
*   **[`tui/`](file:///d:/Project%20Operon/Operon/tui)**: The terminal user interface (TUI) client built on **Ratatui** for developers who prefer running the agent within their terminal.
*   **[`landing/`](file:///d:/Project%20Operon/Operon/landing)**: The project's marketing and informational landing page.
*   **[`scripts/`](file:///d:/Project%20Operon/Operon/scripts)**: Collection of utility runner scripts for Windows and cross-platform environments.

---

## Prerequisites & Development Setup

Before you can build and run Operon locally, make sure you have the following installed on your machine:

### 1. System Requirements
*   **Rust**: Install the stable Rust toolchain via [rustup](https://rustup.rs/).
*   **Node.js & npm** (Only for GUI development): Required to run the Tauri dev server and manage web frontend scripts.
*   **Tauri Prerequisites**:
    *   *Windows*: Install Visual Studio Build Tools (C++ build tools) and the C++ Clang Compiler.
    *   *macOS / Linux*: Refer to the [Tauri Prerequisites Guide](https://v2.tauri.app/start/prerequisites/) to install dependencies such as `webkit2gtk`, `libgtk-3`, and `patchelf`.

### 2. Cloning the Repository
Operon depends on recursive submodules for some of its components. Clone the project using:

```bash
git clone --recursive https://github.com/lukagray-dev/Operon.git
cd Operon
```

If you have already cloned the repository without submodules, run:

```bash
git submodule update --init --recursive
```

---

## Development Workflows

### Developing the GUI Client (`gui/`)
The GUI runs on Tauri, which hot-reloads the UI when frontend or Rust files are modified.

1.  Navigate to the GUI folder and install dependencies:
    ```bash
    cd gui
    npm install
    ```
2.  Start the Tauri development server:
    ```bash
    npm run dev
    ```
    *This compiles the Tauri Rust core and spawns the native application window.*

### Developing the TUI Client (`tui/`)
To run the terminal client:

```bash
cargo run -p operon-tui
```
*(On Windows, you can also use the helper scripts under `scripts/run-tui.bat`)*

### Running Workspace Tests
Before committing, make sure all tests pass:

```bash
cargo test --workspace --exclude operon-tui --exclude operon-gui
```

---

## Coding & Design Standards

To maintain a production-grade codebase, please adhere to the following principles:

### 1. Premium Design Aesthetics
*   **No Placeholders**: Never include empty placeholders, broken links, or empty mock screens.
*   **No Emojis in UI**: When building GUI elements, use professional-grade vector SVGs, PNGs, or drawables. Emojis look different across operating systems and detract from the premium feel.
*   **Vibrant Styles**: Use HSL color schemes, curated dark-mode palettes, and modern typography (e.g., *Inter*, *PT Serif* or *Outfit*).

### 2. Production Code Quality
*   **Detailed Inline Comments**: Write comments that explain the **why** behind complex decisions. Imagine you are explaining the code to a newbie developer friend. Explain assumptions, edge cases, and design constraints.
*   **Deterministic Logic**: Design functions to be predictable, well-structured, and strictly type-safe. Avoid unsafe blocks or raw unwraps unless thoroughly documented and tested.
*   **Separation of Concerns**: Ensure layers are well-separated. Keep Tauri frontend controllers, Tauri system commands, and `operon-rs` logic completely separate.

### 3. Permissions Security Model
Operon has a strict, directory-scoped security model (split between *Owner* and *External* roles). If you add a new tool or system API:
*   Ensure it checks the configuration permissions schema.
*   Ensure the tool is scoped only to allowed directories.
*   Document if the tool requires user confirmation (`Ask` mode) or can run automatically (`Allow`).

---

## Submitting Your Contribution

We follow a typical feature-branch workflow. Please follow these steps to submit your changes:

### 1. Branch Naming
Create a new branch off the `dev` branch:
*   Features: `feat/your-feature-name`
*   Bug fixes: `fix/bug-description`
*   Documentation: `docs/what-changed`

### 2. Commit Guidelines
We use semantic commit messages to automatically build our changelogs. Please format your commit messages like this:

*   `feat(gui): implement custom sidebar chevron resizing`
*   `fix(core): resolve nvidia nim api model discovery crash`
*   `docs: update contributing guidelines`
*   `refactor(tui): split state machine into screen controllers`

### 3. Submitting the PR
1.  Push your branch to your fork:
    ```bash
    git push origin feat/your-feature-name
    ```
2.  Open a Pull Request pointing **to the `dev` branch** of the main Operon repository.
3.  **Do not merge to `main` directly**: The `main` branch is reserved for releases. Merging into `main` automatically triggers the release workflow and compiles binaries. 

---

Thank you for contributing to Operon! If you have any questions, feel free to open a discussion or reach out to us at `heylukagray@gmail.com`.
