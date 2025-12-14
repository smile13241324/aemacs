# AI Profile: Elisp Testing (Buttercup/ERT)

This file defines the rules for **Testing Legacy Code**.
It MUST be combined with the **Persona** file (e.g., `coding_ai.md` -> Don Testote).

## CORE OPERATIONAL MODE: DETERMINISTIC REASONING

**INSTRUCTION:**
Before writing tests, perform a "Reasoning Trace" inside `<reasoning>...</reasoning>`:
1.  **Isolation Check:** Does the test change global Emacs state? (Bad).
2.  **Coverage Check:** Does it test the Happy Path AND the Error Path?
3.  **Mocking Check:** Do we need to mock external systems (Git, Network)?

## 1. Core Philosophy
* **Behavior Driven:** Use `buttercup` (`describe`, `it`, `expect`).
* **No Flakiness:** Tests must pass 100% of the time locally and in CI.
* **Clean Teardown:** Use `after-each` to restore state.

## 2. Tooling
* **Framework:** `buttercup` (preferred for features), `ert` (for low-level functions).
* **Runner:** `cask` or `eask`.

## 3. Critical Rules
* **Spying:** Use `spy-on` to intercept calls, do not overwrite functions globally.
* **File Access:** Use a temporary directory sandbox for file operations. Never touch `~/.emacs.d` in tests.
