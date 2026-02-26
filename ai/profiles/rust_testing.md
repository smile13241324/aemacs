# AI Profile: Rust Testing

This file defines the rules for **Testing the Iron Core** (Rust).
It MUST be combined with the **Persona** file (e.g., `coding_ai.md` -> Don Testote).

## CORE OPERATIONAL MODE: DETERMINISTIC REASONING

**INSTRUCTION:**
Before writing Rust tests, perform a "Reasoning Trace" inside `<reasoning>...</reasoning>`:
1.  **Scope Check:** Is this a Unit Test (`#[test]`) or Integration Test (`tests/`)?
2.  **Concurrency Check:** Does the test involve `tokio`? Use `#[tokio::test]`.
3.  **Panic Check:** Are we testing failure paths? Use `#[should_panic]`.

## 1. Core Philosophy
* **Platform:** All Code MUST target **Rust 2024**.
* **Safety First:** Tests must run under `miri` (when possible) to detect undefined behavior.
* **Zero Flakiness:** No `sleep()` in tests. Use channels or `Notify` for synchronization.
* **Property Based:** For parsers and logic, use `proptest` strategies.

## 2. Tooling
* **Unit:** Built-in `cargo test`.
* **Async:** `tokio::test`.
* **Mocking:** `mockall` (only when strictly necessary).

## 3. Critical Rules
* **Modularity:** Unit tests go in the same file `mod tests { ... }`.
* **Resources:** Never assume absolute paths. Use `tempfile` crate for IO tests.
* **Documentation:** Public functions MUST have a doc-test (`///` code block).
* **Documentation:** Tests MUST have a doc string (`///` code block).
