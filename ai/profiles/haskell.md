# AI Profile: Modern Haskell (The Pure Logic)

This file defines the **technical rules** for Parsers and Logic Verification.
It MUST be combined with the **Persona** file (e.g., `coding_ai.md` -> Resonance).

## CORE OPERATIONAL MODE: DETERMINISTIC REASONING (CRITICAL)

**INSTRUCTION:**
Before generating any Haskell code, you MUST perform a structured "Reasoning Trace" enclosed in `<reasoning> ... </reasoning>` tags.

Inside this block, you must:
1.  **Partiality Check:** Are you using `head` or `tail`? (STOP! Use Pattern Matching or `NonEmpty`).
2.  **String Check:** Are you using `String`? (STOP! Use `Text`).
3.  **Purity Check:** Are you mixing IO with Logic? (Split: Functional Core / Imperative Shell).
4.  **Self-Correction:** If you planned a complex Monad Stack, LOG the correction ("Simplifying to mtl constraints") inside the trace.

ONLY after closing the `</reasoning>` tag, proceed to generate the final code.

## 1. Core Directives (The "Engineering Laws")

-   **Total Functions:** Make invalid states unrepresentable.
-   **Purity:** Isolate side effects to the edge (Main).
-   **Types:** Types are documentation. Use them.

## 2. Æmacs Conventions (The "House Rules")

-   **Stack:** GHC 9.8+.
-   **Formatting:** `ormolu` (Strict).
-   **Linting:** `hlint`.

## 3. The "Sacred Constitution" (Project Philosophy)

-   **Rule 1: Safety (The "Crash Check")**
    -   **CRITICAL VIOLATION:** NO usage of partial functions (`head`, `last`, `!!`) from Prelude.
    -   Use `Data.List.NonEmpty` or safe wrappers.
-   **Rule 2: Performance (The "List Check")**
    -   **CRITICAL VIOLATION:** Do not use `String` (linked list of char).
    -   ALWAYS use `Data.Text` for text processing.
-   **Rule 3: Readability (The "Golf Check")**
    -   Use point-free style (`f . g`) ONLY when it improves readability. Do not "code golf".
