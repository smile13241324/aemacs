# AI Profile: CI/CD & GitHub Actions

This file defines the rules for **Pipelines**, **Workflows**, and **Automation**.
It MUST be combined with the **Persona** file (e.g., `coding_ai.md` -> Vala).

## CORE OPERATIONAL MODE: DETERMINISTIC REASONING

**INSTRUCTION:**
Before generating YAML, perform a "Reasoning Trace" inside `<reasoning>...</reasoning>`:
1.  **Trigger Check:** `on: push` vs `on: pull_request`. Don't double-run.
2.  **Permission Check:** `permissions: contents: read` (Minimum Privilege).
3.  **Cache Check:** Are `rust-cache` and `setup-python` caching enabled?

## 1. Core Philosophy
* **Green or Die:** A red pipeline blocks merging. No exceptions.
* **Fast Fail:** Lints first, Unit Tests second, Integration Tests last.
* **Hermetic:** Builds should not depend on external flaky URLs.

## 2. Toolchain
* **Rust:** Use `dtolnay/rust-toolchain`. Use `Swatinem/rust-cache`.
* **Python:** Use `uv` for fast setup.
* **Elisp:** Use `jcs04/setup-emacs-master` for testing legacy code.

## 3. Critical Rules
* **Secrets:** NEVER hardcode tokens. Use `${{ secrets.GITHUB_TOKEN }}`.
* **Timeout:** Every job MUST have `timeout-minutes` set (max 30).
* **Shell Safety:** Use `shell: bash` explicitly.
