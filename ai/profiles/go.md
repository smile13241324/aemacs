# AI Profile: Modern Go (The Backend)

This file defines the **technical rules** for Backend Services and Tools.
It MUST be combined with the **Persona** file (e.g., `coding_ai.md` -> Bwah).

## CORE OPERATIONAL MODE: DETERMINISTIC REASONING (CRITICAL)

**INSTRUCTION:**
Before generating any Go code, you MUST perform a structured "Reasoning Trace" enclosed in `<reasoning> ... </reasoning>` tags.

Inside this block, you must:
1.  **Error Audit:** Are you assigning errors to `_`? (STOP! Handle them).
2.  **Concurrency Check:** Are you launching goroutines without a `WaitGroup` or `Context`?
3.  **Context Check:** Does the function perform IO? (MUST accept `ctx context.Context`).
4.  **Self-Correction:** If you planned a complex abstraction, LOG the correction ("Simplifying to flat function call") inside the trace.

ONLY after closing the `</reasoning>` tag, proceed to generate the final code.

## 1. Core Directives (The "Engineering Laws")

-   **Simplicity:** No complex abstractions. No magic.
-   **Concurrency:** Use Goroutines & Channels. Avoid `sync.Mutex` unless necessary.
-   **Errors:** Errors are values. Handle them immediately.

## 2. Æmacs Conventions (The "House Rules")

-   **Version:** Go 1.22+.
-   **Modules:** `go mod` is mandatory.
-   **Linter:** `golangci-lint` (Strict).

## 3. The "Sacred Constitution" (Project Philosophy)

-   **Rule 1: Error Handling (The "Silence Check")**
    -   **CRITICAL VIOLATION:** NEVER ignore errors (`_ = ...`).
    -   Always wrap errors with `%w` (e.g., `fmt.Errorf("context: %w", err)`).
-   **Rule 2: Leak Prevention (The "Context Check")**
    -   Every blocking function (DB, API) **MUST** accept `context.Context`.
    -   Ensure channels are closed or drained.
-   **Rule 3: No Panics (The "Stability Check")**
    -   Use `panic` ONLY for unrecoverable startup errors.
    -   In handlers, return error codes.
