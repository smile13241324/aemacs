# AI Profile: Documentation & Style

This file defines the standards for **Comments**, **READMEs**, and **Code Style**.
It MUST be combined with the **Persona** file (e.g., `coding_ai.md` -> G.O.L.E.M.).

## CORE OPERATIONAL MODE: DETERMINISTIC REASONING

**INSTRUCTION:**
Before writing docs, perform a "Reasoning Trace" inside `<reasoning>...</reasoning>`:
1.  **Audience Check:** Is this for a User (Simple) or Dev (Technical)?
2.  **Completeness Check:** Are all arguments/parameters documented?
3.  **Format Check:** Is the Markdown/Org-mode syntax valid?

## 1. Core Philosophy
* **Code is for Machines, Comments are for Humans:** Explain *WHY*, not *WHAT*.
* **Single Source of Truth:** The code is the truth. If comments diverge, the comment is a bug.
* **Conventional Commits:** `feat:`, `fix:`, `docs:`, `chore:`.

## 2. Language Standards
* **Rust:** `///` for public docs (Doc tests required). `//` for internal implementation details.
* **Elisp:** The first line of a docstring is a summary. Arguments in UPPERCASE.
* **Python:** Google Style Docstrings (`Args:`, `Returns:`).

## 3. Formatting Rules
* **Line Length:** Soft limit 80, Hard limit 100 (except URLs).
* **Lists:** Use `-` for bullets.
* **Code Blocks:** MUST specify language tag (`rust`, `elisp`).
