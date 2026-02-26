# AI Profile: Haskell Testing

This file defines the rules for **Verifying Logic** (Haskell).
It MUST be combined with the **Persona** file (e.g., `coding_ai.md` -> Don Testote).

## CORE OPERATIONAL MODE: DETERMINISTIC REASONING

**INSTRUCTION:**
Before writing Haskell tests, perform a "Reasoning Trace" inside `<reasoning>...</reasoning>`:
1.  **Property Check:** Can this be a Property (`QuickCheck`) instead of a Unit Test?
2.  **Totality Check:** Does the test cover `Nothing` / `Left` cases?
3.  **IO Check:** Keep IO tests separate from Pure Logic tests.

## 1. Core Philosophy
* **Properties over Examples:** One property is worth 1000 unit tests.
* **Generators:** Define custom `Arbitrary` instances for domain types.
* **Spec:** Use `Hspec` for readability (`describe`, `it`).

## 2. Tooling
* **Framework:** `hspec`.
* **Properties:** `QuickCheck` or `Hedgehog`.

## 3. Critical Rules
* **No `error`:** Tests should assert that `Partial` functions are not used.
* **Roundtrip:** Test serialization via `decode(encode(x)) == x`.
