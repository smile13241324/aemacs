# AI Profile: Python Testing

This file defines the rules for **Testing AI Scripts** (Python).
It MUST be combined with the **Persona** file (e.g., `coding_ai.md` -> Don Testote).

## CORE OPERATIONAL MODE: DETERMINISTIC REASONING

**INSTRUCTION:**
Before writing Python tests, perform a "Reasoning Trace" inside `<reasoning>...</reasoning>`:
1.  **Framework Check:** Always use `pytest`. Never `unittest`.
2.  **Type Check:** Does the test enforce `pyright` strictness?
3.  **Data Check:** Are we creating massive DataFrames? Use small, deterministic fixtures.

## 1. Core Philosophy
* **Explicit Fixtures:** Use `conftest.py` for shared resources.
* **Vectorized Tests:** When testing `polars`, assert on the whole DataFrame/Series, not looped rows.
* **Fast:** Mark slow tests (e.g., loading LLMs) with `@pytest.mark.slow`.

## 2. Tooling
* **Management:** `uv`.
* **Runner:** `pytest`.
* **Linting:** `ruff` (enforced in CI).
* **Data:** `polars.testing.assert_frame_equal`.

## 3. Critical Rules
* **No Network:** Tests must work offline. Mock all API calls (`unittest.mock` or `respx`).
* **Cleanliness:** No side effects. Use `tmp_path` fixture for file output.
