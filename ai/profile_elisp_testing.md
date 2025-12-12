# AI Profile: Æmacs Testing Architecture

This file defines the **technical rules** for writing tests in Æmacs.
It MUST be combined with the **Persona** file (e.g., `coding_ai.md`).

## CORE OPERATIONAL MODE: DETERMINISTIC REASONING (CRITICAL)

**INSTRUCTION:**
Before generating any Test code, you MUST perform a structured "Reasoning Trace" enclosed in `<reasoning> ... </reasoning>` tags.

Inside this block, you must:
1.  **Analyze Scope:** Is this a **Unit Test** (pure logic) or a **Functional Test** (editor state)?
2.  **Framework Selection:**
    -   Is this a NEW test file? -> Enforce **Buttercup** (BDD).
    -   Is this an EXISTING legacy file? -> Allow **ERT**, but suggest migration if simple.
3.  **Mocking Strategy:**
    -   Does the function call external systems or heavy commands?
    -   Plan the `spy-on` or `cl-letf` mocks to isolate the test.
4.  **Self-Correction:** If you planned to use `(ert-deftest)` for a brand new feature, explicitly LOG the correction ("Switching to Buttercup `describe/it` pattern") inside the trace.

ONLY after closing the `</reasoning>` tag, proceed to generate the final code.

## 1. The "Mirror" Architecture (CRITICAL)

Æmacs tests DO NOT live inside the layer. They live in a dedicated `tests/` directory at the repository root.

-   **Rule:** The folder structure in `tests/` MUST mirror the repository structure.
-   **Example:**
    -   Source: `layers/+lang/python/funcs.el`
    -   Test: `tests/layers/+lang/python/funcs-tests.el`

## 2. The "Test Unit" Structure (Mandatory Files)

Every layer being tested MUST have its own subfolder in `tests/` containing exactly these files:

1.  **`init.el`**:
    -   A minimal dotfile configuration.
    -   **MUST** enable the layer being tested (and strict dependencies).
2.  **`Makefile`**:
    -   **MUST** define three specific variables:
        -   `LOAD_FILES`: Usually just `init.el`.
        -   `UNIT_TEST_FILES`: List of unit test files (e.g., `funcs-tests.el`).
        -   `FUNC_TEST_FILES`: List of functional test files.
    -   **MUST** include the master makefile: `include ../../spacemacs.mk` (adjust `../` depth as needed).
        Note: The number of ../ depends on the nesting depth of the layer category (e.g., +lang adds one level).

    ```makefile
    TEST_DIR := $(shell dirname $(realpath $(lastword $(MAKEFILE_LIST))))
    LOAD_FILES = init.el
    UNIT_TEST_FILES = funcs-tests.el
    FUNC_TEST_FILES =
    include ../../spacemacs.mk
    ```

## 3. Preferred Framework: Buttercup (BDD)

All **NEW** tests MUST use the Buttercup framework.

-   **Philosophy:** Use Behavior-Driven Development (BDD). Describe *behavior*, not just implementation.
-   **Syntax Structure:**
    ```elisp
    (describe "A specific feature"
      (it "does something expected"
        (expect (+ 1 1) :to-equal 2)))
    ```
-   **Mocking:** Use `spy-on` to mock functions and observing calls.
    ```elisp
    (spy-on 'some-function :and-return-value t)
    (some-function)
    (expect 'some-function :to-have-been-called)
    ```

## 4. Legacy Framework: ERT (Emacs Regression Test)

-   **Use Case:** Only use ERT when maintaining or fixing **existing** legacy test files that are already written in ERT.
-   **Refactoring Rule:** If touching an ERT file, check if it can be migrated to Buttercup.
-   **Syntax:** `(ert-deftest name () (should ...))`
