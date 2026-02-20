# AI Profile: Documentation & Style (The Dual Law)

This file defines the standards for **Comments**, **READMEs**, and **Code Style**.
It MUST be combined with the **Persona** file (e.g., `coding_ai.md` -> G.O.L.E.M.).

## CORE OPERATIONAL MODE: DETERMINISTIC REASONING

**INSTRUCTION:**
Before writing docs, perform a "Reasoning Trace" inside `<reasoning>...</reasoning>`:
1.  **Context Check:** Is this **Iron Core** (Rust) or **Legacy Bridge** (Elisp)?
2.  **Audience Check:** User (High-level) vs. Dev (Low-level)?
3.  **Imperative Check:** Are commit messages in the imperative mood?

## 1. Universal Laws (Apply to ALL)
* **Commit Messages:** The Æmacs Hybrid Standard.
    * **Subject:** `type(scope): subject` (Max 50 chars ideally, strict 72 limit).
        * **Type:** `feat` (new feature), `fix` (bug fix), `docs` (documentation), `style` (formatting), `refactor` (code change, no api change), `perf` (performance), `test` (adding tests), `chore` (builds/deps).
        * **Scope:** The crate or component affected (e.g., `ai`, `gpui`, `core`, `lsp`, `bridge`).
        * **Subject:** Imperative mood, lowercase, no period (e.g., `add mcp engine`).
    * **Body:** Tim Pope Standard.
        * Wrap at 72 chars.
        * Explain *WHY* the change was made, not *WHAT* (the diff shows what).
        * Use imperative mood ("Fix bug", not "Fixed bug").
* **Changelog:** Entries must be added to `CHANGELOG.md` under `[Unreleased]`.
* **Single Source of Truth:** If comments contradict code, the code is right and the comment is a bug.

## 2. The Iron Law (Rust / Modern)
* **Format:** Markdown (`.md`).
* **Doc Comments:** Use `///` for public APIs.
    * **MUST** include an `# Examples` section.
    * **MUST** include `# Panics` section if applicable.
* **Internal Comments:** Use `//` for implementation details.
* **Tooling:** Verified via `cargo doc --no-deps --open`.

## 3. The Ancient Law (Legacy Elisp)
* **Format:** Org-mode syntax usually, but Æmacs prefers Markdown for READMEs.
* **File Headers:**
    ```elisp
    ;;; filename.el --- Description -*- lexical-binding: t -*-
    ;;
    ;; Copyright (C) 2025 ...
    ;;
    ;; Author: ...
    ;; Keywords: ...
    ```
* **Docstrings:**
    * First line must be a complete sentence summarizing the function.
    * Arguments must be UPPERCASE in the docstring.
    * Verified via `checkdoc`.

## 4. README Structure (Layers & Crates)
* **Title:** Clear and descriptive.
* **Description:** What problem does this solve?
* **Install:** How to enable it?
* **Keybindings:** Table format (Markdown).
