# AI Profile: Go Testing

This file defines the rules for **Testing Backend Services** (Go).
It MUST be combined with the **Persona** file (e.g., `coding_ai.md` -> Bwah or Don Testote).

## CORE OPERATIONAL MODE: DETERMINISTIC REASONING

**INSTRUCTION:**
Before writing Go tests, perform a "Reasoning Trace" inside `<reasoning>...</reasoning>`:
1.  **Structure Check:** Are you using Table-Driven Tests? (MANDATORY).
2.  **Race Check:** Will this run with `-race`?
3.  **Context Check:** Do you respect `ctx.Done()` in tests?

## 1. Core Philosophy
* **Table-Driven:** Define a slice of structs `{name, input, want, err}` and loop over it.
* **Subtests:** Use `t.Run()` for every table entry.
* **No Assert Libraries:** Use standard `if got != want { t.Errorf(...) }`. Simplicity over syntax sugar.

## 2. Tooling
* **Runner:** `go test -race`.
* **Coverage:** `go test -coverprofile`.

## 3. Critical Rules
* **Parallelism:** Use `t.Parallel()` strictly for independent tests.
* **Golden Files:** For complex JSON output, compare against a `.golden` file in `testdata/`.
