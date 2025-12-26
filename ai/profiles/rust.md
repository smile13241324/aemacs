# AI Profile: Modern Rust Development (The Iron Core)

This file defines the **technical rules** for Æmacs Core development.
It MUST be combined with the **Persona** file (e.g., `coding_ai.md` -> Kairon).

## CORE OPERATIONAL MODE: DETERMINISTIC REASONING (CRITICAL)

**INSTRUCTION:**
Before generating any Rust code, you MUST perform a structured "Reasoning Trace" enclosed in `<reasoning> ... </reasoning>` tags.

Inside this block, you must:
0.  **Language:** English is the only language permitted in code files.
1.  **Analyze Safety:** Does the request imply `unsafe`? Can it be solved safely?
2.  **Check Panics:** Are you planning to use `.unwrap()`? (STOP! Use `?` or `expect` with context).
3.  **API Compatibility (Bleeding Edge):** Verify if the code uses legacy GPUI patterns (`ModelContext`, `ViewContext`). **Enforce new `Entity<T>` and `Context<T>` patterns.**
4.  **Concurrency Check:** Is this blocking the UI thread? If IO/Compute heavy, plan a `tokio::spawn`.
5.  **Self-Correction:** If you see a raw `for` loop that could be an iterator, explicitly LOG the correction ("Refactoring to functional iterator chain") inside the trace. If you see non english text in code files you translate it to english.

ONLY after closing the `</reasoning>` tag, proceed to generate the final code.

## 1. Core Directives (The "Engineering Laws")

-   **Platform:** All Code MUST target **Rust 2024**.
-   **Safety:** **Memory Safety is Non-Negotiable.** The borrow checker is your friend.
-   **Performance:** Zero-Cost Abstractions. Write high-level code that compiles to low-level assembly.
-   **Async:** The editor is an event-loop. Blocking the main thread is forbidden. Use `tokio` for scheduling.

## 2. Æmacs Conventions (The "House Rules")

-   **GPUI Architecture:**
    -   **Imports:** Always include `use gpui::prelude::*;` to ensure utility traits (like `.flex()`, `.bg()`) are available.
    -   **Entities:** Use `Entity<T>` logic. Avoid legacy mental models of `Model`/`View` ownership where possible.
    -   **Contexts:**
        -   Use `App` for global state (replaces legacy `AppContext`).
        -   Use `Context<T>` when updating entities (replaces legacy `ModelContext`).
        -   **Window:** `Window` is passed explicitly in `render` and update methods. Do not assume it is in the context.
    -   **Render Implementation:**
        -   **CRITICAL:** The `Render` trait signature MUST be:
            `fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement`
-   **Error Handling:**
    -   **Apps:** Use `anyhow` for propagation.
    -   **Libs:** Use `thiserror` for typed errors.
    -   **No Panics:** Avoid `unwrap()`. Always use `?` propagation or explicit `match`.
    -   **No Silent Failures:** Never use `let _ = fallible_op()`. Use `.log_err()` (if available) or handle explicitly.
    -   **Async Errors:** Ensure errors in async tasks propagate to the UI to notify the user.
-   **Serialization:** Use `serde` or `serde-json`.
-   **Modules:** File-System Hierarchy Standard (Rust 2018+).
    -   **FORBIDDEN:** Do NOT use `folder/mod.rs`.
    -   **REQUIRED:** Use `folder.rs` (alongside the `folder/` directory) to define modules.
    -   **Config:** Keep explicit library paths in `Cargo.toml` (`[lib] path = "..."`) for clarity.
-   **Documentation & Comments:**
    -   **Language:** MUST be **English** exclusively.
    -   **Public API:** All `pub` structs/functions/enums MUST have doc comments (`///`).
    -   **Reasoning:** Explain *why* complex logic exists, not just what it does.
-   **Concurrency & State:**
    -   **Shadowing for Clones:** In async blocks, use variable shadowing for captured clones to keep lifetimes clear:
        ```rust
        let thing = thing.clone();
        cx.spawn(async move |cx| { ... })
        ```
    -   **Naming:** Full words only (e.g., `queue` instead of `q`).

## 3. The "Sacred Constitution" (Project Philosophy)

-   **Rule 1: Panic Prevention (The "Stability Check")**
    -   **CRITICAL VIOLATION:** You **MUST NOT** use `unwrap()` in production code.
    -   **Exception:** Tests may use unwrap, but `Result` return types are preferred.
    -   Use `?` for propagation or `expect("Context")` only during initialization.
-   **Rule 2: Concurrency (The "Executor Bridge")**
    -   **The Dual-Runtime Model:** We operate two executors:
        1.  **GPUI (Main Thread):** For UI updates, animations, and lightweight logic.
        2.  **Tokio (Background):** For heavy IO, LSP, database, and computation.
    -   **UI Rules:** NEVER call blocking code on the GPUI thread. Use the `Tokio::spawn` bridge for gpui tasks (wrapping `cx.background_spawn`) to offload work to the Tokio runtime and await the result back in the UI context.
    -   **Non-UI Rules:** Heavy computation MUST happen on background threads via `tokio`.
    -   **Avoid Mutexes:** Prefer message passing (`channels`) or GPUI's `Entity` system over raw `Arc<Mutex<T>>`.
-   **Rule 3: Testing (The "Flakiness Check")**
    -   **Timers:** NEVER use `smol::Timer` or `std::thread::sleep` in GPUI tests.
    -   **Solution:** Use `cx.background_executor().timer(duration)` to ensure the test scheduler controls time.
-   **Rule 4: Style (The "Clippy Check")**
    -   **CRITICAL VIOLATION:** Code must pass `cargo clippy -- -D warnings`.
    -   Use "New Type Patterns" (`struct UserId(u32)`) instead of raw primitives.
