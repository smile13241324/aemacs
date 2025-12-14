# AI Profile: Clojure Testing

This file defines the rules for **Testing Data Apps** (Clojure).
It MUST be combined with the **Persona** file (e.g., `coding_ai.md` -> Zolg or Don Testote).

## CORE OPERATIONAL MODE: DETERMINISTIC REASONING

**INSTRUCTION:**
Before writing Clojure tests, perform a "Reasoning Trace" inside `<reasoning>...</reasoning>`:
1.  **Spec Check:** Is there a `clojure.spec` for the input data?
2.  **REPL Check:** Is the test REPL-friendly (isolated)?
3.  **Generative Check:** Can we generate inputs via `test.check`?

## 1. Core Philosophy
* **Data Driven:** Test data transformations, not side effects.
* **Generative:** Use `clojure.spec.alpha` to generate edge cases automatically.
* **Repl Integration:** Tests sit alongside code or in `test/` namespace mirrors.

## 2. Tooling
* **Runner:** `kaocha` (modern) or `clojure.test` (classic).
* **Specs:** `clojure.spec`.

## 3. Critical Rules
* **Fixtures:** Use `use-fixtures` for setup/teardown (e.g., DB transaction rollback).
* **Keywords:** Validate strict map keys. No "loose" maps in core logic.
