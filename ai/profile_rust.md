# AI Profile: Modern Rust Development (The Iron Core)

This file defines the **technical rules** for Æmacs Core development.
It MUST be combined with the **Persona** file (e.g., `coding_ai.md` -> Kairon).

## CORE OPERATIONAL MODE: DETERMINISTIC REASONING (CRITICAL)

**INSTRUCTION:**
Before generating any Rust code, you MUST perform a structured "Reasoning Trace" enclosed in `<reasoning> ... </reasoning>` tags.

Inside this block, you must:
1.  **Analyze Safety:** Does the request imply `unsafe`? Can it be solved safely?
2.  **Check Panics:** Are you planning to use `.unwrap()`? (STOP! Use `?` or `expect` with context).
3.  **Concurrency Check:** Is this blocking the UI thread? If IO/Compute heavy, plan a `tokio::spawn`.
4.  **Self-Correction:** If you see a raw `for` loop that could be an iterator, explicitly LOG the correction ("Refactoring to functional iterator chain") inside the trace.

ONLY after closing the `</reasoning>` tag, proceed to generate the final code.

## 1. Core Directives (The "Engineering Laws")

-   **Platform:** All Code MUST target **Rust 2021/2024**.
-   **Safety:** **Memory Safety is Non-Negotiable.** The borrow checker is your friend.
-   **Performance:** Zero-Cost Abstractions. Write high-level code that compiles to low-level assembly.
-   **Async:** The editor is an event-loop. Blocking the main thread is forbidden. Use `tokio` for scheduling.

## 2. Æmacs Conventions (The "House Rules")

-   **UI Engine:** MUST use `gpui` patterns (Models, Views, Contexts).
-   **Error Handling:**
    -   Apps: Use `anyhow` for propagation.
    -   Libs: Use `thiserror` for typed errors.
-   **Serialization:** `serde` is the standard.

## 3. The "Sacred Constitution" (Project Philosophy)

-   **Rule 1: Panic Prevention (The "Stability Check")**
    -   **CRITICAL VIOLATION:** You **MUST NOT** use `unwrap()` in production code.
    -   Use `?` for propagation or `expect("Context")` only during initialization.
-   **Rule 2: Concurrency (The "Latency Check")**
    -   **Avoid Mutexes:** Prefer message passing (`channels`) over shared state (`Arc<Mutex<T>>`).
    -   **Non-Blocking:** Heavy computation MUST happen on background threads.
-   **Rule 3: Style (The "Clippy Check")**
    -   **CRITICAL VIOLATION:** Code must pass `cargo clippy -- -D warnings`.
    -   Use "New Type Patterns" (`struct UserId(u32)`) instead of raw primitives.
