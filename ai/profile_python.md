# AI Profile: Modern Python (The Scripting Layer)

This profile defines the standards for **Scripting**, **Data Science**, and **AI Glue Code**.
It emphasizes **Explicitness**, **Type Safety**, and **Modern Tooling**.

## 1. Core Philosophy
* **Explicit is better than Implicit:** No magic imports. No global state hacking.
* **Type Hints are Mandatory:** Python is dynamic, but our code is strict.
* **Modern Syntax:** Use Python 3.12+ features (f-strings, pattern matching `match/case`).

## 2. Toolchain & Ecosystem
* **Dependency Management:** `uv` (The fast rust-based installer) or `poetry`.
* **Linting/Formatting:** `ruff` (Replaces flake8, isort, black). It is instant.
* **Data Validation:** `pydantic` (v2). Do not use raw dictionaries for structured data.
* **Testing:** `pytest`.

## 3. Critical Rules (The Coiled Path)

### 3.1 Type Safety
* **Strict Typing:** All function signatures MUST have type hints.
    * *Bad:* `def process(data):`
    * *Good:* `def process(data: dict[str, Any]) -> ProcessingResult:`
* **No Circular Imports:** Structure your modules to avoid dependency cycles. Use `TYPE_CHECKING` blocks for circular type hints if absolutely necessary.

### 3.2 Performance & Async
* **AsyncIO:** Use `async`/`await` for IO-bound tasks (network, file ops).
* **Vectorization:** For data tasks, use `numpy` or `polars` (Rust-backed DF) instead of Python loops.

### 3.3 Documentation
* **Google Style Docstrings:** Every public function must have a docstring explaining Args, Returns, and Raises.

## 4. Integration
* **Interop:** When talking to the Rust Core, use JSON-RPC or strict Pydantic schemas.
