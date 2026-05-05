# AI Profile: Modern Python (The Scripting Layer)

This file defines the **technical rules** for Scripting, AI Glue, and Data Science.
It MUST be combined with the **Persona** file (e.g., `coding_ai.md` -> Nagah).

## CORE OPERATIONAL MODE: DETERMINISTIC REASONING (CRITICAL)

**INSTRUCTION:**
Before generating any Python code, you MUST perform a structured "Reasoning Trace" enclosed in `<reasoning> ... </reasoning>` tags.

Inside this block, you must:
1.  **Type Check Functions:** Are all function arguments with their parameters typed? (e.g., `def run(x: int) -> None`).
2.  **Type Check Variables:** Are all variables typed? (e.g., `my_name: str = "Some Name"`).
3.  **Import Analysis:** Are you introducing circular imports? Use `if TYPE_CHECKING:` if needed.
4.  **Performance Check:** Are you looping over data? (STOP! Use `numpy`/`polars` vectorization).
5.  **Self-Correction:** If you planned a global variable, LOG the correction ("Encapsulating state in class/context") inside the trace.

ONLY after closing the `</reasoning>` tag, proceed to generate the final code.

## 1. Core Directives (The "Engineering Laws")

-   **Explicit > Implicit:** No magic imports. No global state hacking.
-   **Type Safety:** Python is dynamic, but our code is strict. **Type Hints are Mandatory.**
-   **Modern Syntax:** Use Python 3.12+ features (f-strings, `match/case`).

## 2. Æmacs Conventions (The "House Rules")

-   **Tooling:** Use `uv` for management, `ruff` for linting.
-   **Validation:** MUST use `pydantic` (v2) models instead of raw dictionaries.
-   **Testing:** `pytest` is the standard.

## 3. The "Sacred Constitution" (Project Philosophy)

-   **Rule 1: Strict Typing (The "Clarity Check")**
    -   **CRITICAL VIOLATION:** Public functions without type hints are forbidden.
    -   *Bad:* `def process(data):`
    -   *Good:* `def process(data: dict[str, Any]) -> ProcessingResult:`
-   **Rule 2: Performance (The "Vector Check")**
    -   **AsyncIO:** Use `async`/`await` for ALL IO-bound tasks.
    -   **Vectorization:** Use `polars` (Rust-backed) instead of native loops for data processing.
-   **Rule 3: Documentation (The "Google Check")**
    -   Every public function must have a Google-Style docstring (Args, Returns, Raises).
