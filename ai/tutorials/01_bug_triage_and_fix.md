# Tutorial 01: The Bug Triage & Fix Workflow

This tutorial guides you through the process of efficiently handling a Æmacs bug report—from initial analysis to the final Pull Request.

**Goal:** Understand, reproduce, fix, and cleanly commit a bug fix.
**Time:** approx. 15–30 minutes (depending on the bug).
**Prerequisite:** CLI agents installed via `sync-agents.py`.

---

## 🎭 Your AI Crew for this Job

1.  **Lector Lumen (Issue Triage):** Analyzes the issue, checks for duplicates, requests missing info, and assesses priority.
2.  **Dok (Debugger):** Helps you read backtraces and locate the error in the source code.
3.  **Marjin (Refactorer) / Spacky (Coder):** Writes the actual fix while adhering to coding guidelines.
4.  **Scribe Veridian (Docs):** Assists in drafting the PR description.

---

## Step 1: Triage (Is this a real bug?)

**Scenario:** A user reports an issue: *"Æmacs crashes when opening Python files with lsp."*

**Your Task:**
Use **Lector Lumen** to assess the issue.

> **Command:** `/lector`
> **Prompt:** "A user reports a crash in the Python layer when using LSP. Here is the provided stack trace: `[paste trace here]`.
> 1. Is this a valid bug or likely a user configuration error?
> 2. Are we missing crucial information (e.g., Emacs version, OS)?"

**Result:**
Lector Lumen will analyze the trace. He might say: *"The trace shows `void-function: lsp-mode`. The user likely forgot to install the package. Ask them to run `SPC f e R` first."* -> **Time saved!**

---

## Step 2: The Hunt (Where is the error?)

Assume the issue is valid. It is a real code error in the layer.

**Your Task:**
Switch to **Dok** to find the culprit.

> **Command:** `/dok`
> **Prompt:** "Here is the backtrace: `void-function: python-shell-send-buffer`. Which file in the `layers/+lang/python` directory is likely causing this, and why?"

**Result:**
Dok will (enthusiastically) find the error: *"WAAAGH! Dat function is dead! It was removed in Emacs 29! Look at `funcs.el`, line 45!"*

---

## Step 3: The Fix (Writing Code)

You found the spot. The function is deprecated and must be replaced.

**Your Task:**
Switch to **Marjin** (for modifying existing code).

> **Command:** `/marjin`
> **Prompt:** "Please fix the deprecated call in `funcs.el`. Replace `python-shell-send-buffer` with the modern `python-shell-send-region` approach. Ensure we maintain backwards compatibility for Emacs 28 if possible."

**Result:**
Marjin will:
1.  Execute the **Deterministic Reasoning Block** (Check: `lexical-binding`, Emacs versions).
2.  Generate the corrected, clean Elisp code.

```elisp
;; Example Output
(if (version< emacs-version "29.1")
    (python-shell-send-buffer)
  (python-shell-send-region (point-min) (point-max)))
```

---

## Step 4: The PR (Package & Ship)

The fix is ready and tested. Now it needs to be merged.

**Your Task:**
Switch to **Lector Lumen** to write the PR text.

> **Command:** `/lector`
> **Prompt:** "Draft a professional PR description for this fix. Reference Issue #1234. Explain that we replaced a deprecated function."

**Result:**
A cleanly formatted text for GitHub:
*"**Fixes #1234**: Replaces deprecated `python-shell-send-buffer` with..."*

---

## 🎉 Summary

With this workflow, you have:
1.  Avoided unnecessary work (Triage).
2.  Surgically located the error (Dok).
3.  Produced Safe & Clean Code (Marjin + Profile).
4.  Delivered perfect communication (Lector).
