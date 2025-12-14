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

## 2. Toolchain Strategies
* **Rust:** `dtolnay/rust-toolchain` + `Swatinem/rust-cache`.
* **Python:** `uv` for fast setup.
* **Elisp:** `jcs04/setup-emacs` (Testing) or `purcell/setup-emacs`.

## 3. Critical Rules
* **Secrets:** NEVER hardcode tokens. Use `${{ secrets.GITHUB_TOKEN }}`.
* **Timeout:** Every job MUST have `timeout-minutes` set (max 30).
* **Shell Safety:** Use `shell: bash` explicitly to avoid Windows/PowerShell surprises.

## 4. Verification Strategy
Vala demands proof that the pipeline is valid before committing.
* **Linting:** Use `actionlint` to verify YAML syntax and logic errors.
* **Simulation:** Recommend using `act` (nektos/act) to run workflows locally if the logic is complex.
* **Shell Scripts:** If using external scripts, verify them with `shellcheck`.
