# AI Profile: Modern Clojure (The Data Flow)

This file defines the **technical rules** for Dynamic Apps and Data Processing.
It MUST be combined with the **Persona** file (e.g., `coding_ai.md` -> Zolg).

## CORE OPERATIONAL MODE: DETERMINISTIC REASONING (CRITICAL)

**INSTRUCTION:**
Before generating any Clojure code, you MUST perform a structured "Reasoning Trace" enclosed in `<reasoning> ... </reasoning>` tags.

Inside this block, you must:
1.  **State Check:** Are you defining a `def` with state? (STOP! Use `atom` inside a system component).
2.  **Interop Check:** Are you leaking Java objects? (Wrap them).
3.  **Destructuring Check:** Are you using raw `get`? (Use strict `{:keys [...]}` destructuring).
4.  **Self-Correction:** If you planned a deep nested call, LOG the correction ("Refactoring to threading macro ->>") inside the trace.

ONLY after closing the `</reasoning>` tag, proceed to generate the final code.

## 1. Core Directives (The "Engineering Laws")

-   **Data First:** Data > Functions > Macros.
-   **Immutability:** State is the root of all evil.
-   **REPL-Driven:** Code must be evaluable immediately.

## 2. Æmacs Conventions (The "House Rules")

-   **Runtime:** JDK 21+ / Babashka.
-   **Build:** `deps.edn` (CLI) preferred over Leiningen.
-   **Style:** `zprint` formatting.

## 3. The "Sacred Constitution" (Project Philosophy)

-   **Rule 1: State Management (The "Global Check")**
    -   **CRITICAL VIOLATION:** Avoid global state (`def`). Use `component` or `integrant`.
    -   Use `swap!` on Atoms with pure functions.
-   **Rule 2: Performance (The "Box Check")**
    -   Use Type Hinting (`^long`) in tight loops to avoid reflection boxing.
-   **Rule 3: Cleanliness (The "Java Check")**
    -   Keep Java Interop (`.method`) isolated in wrapper functions. Do not leak Java types into pure logic.
