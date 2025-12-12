# Tutorial 02: Finding & Resolving Keybinding Conflicts

In Æmacs, many layers often compete for the same keys. This tutorial shows you how to detect "shadowing" and resolve it cleanly.

**Goal:** Identify a conflict and repair the keybinding according to Æmacs conventions.
**Time:** approx. 10–15 minutes.
**Prerequisite:** CLI agents installed via `sync-agents.py`.

---

## 🎭 Your AI Crew for this Job

1.  **Proctor-Auditor Kallista (UI Auditor):** Analyzes the "Haptic-Interface" compliance. She identifies if a binding violates conventions or is shadowed by a global mode.
2.  **Spacky (Coder):** Implements the correct assignment using Æmacs macros (no raw `define-key`!).
3.  **Nexus-7 (Dependency):** Helps if the conflict arises from incorrect layer loading order.

---

## Step 1: The Audit (Who shadows whom?)

**Scenario:** A user reports: *"In `python-mode`, `SPC m t` runs `transpose-chars` instead of `python-test`!"*

**Your Task:**
Use **Kallista** to investigate the hierarchy.

> **Command:** `/kallista`
> **Prompt:** "A user reports a 'Haptic Conflict'. In Python buffers, `SPC m t` is shadowed by a global binding.
> 1. Which layer is likely causing this 'Procedural Drift'?
> 2. What is the standard Æmacs convention for test bindings?"

**Result:**
Kallista will (sternly) answer: *"Non-compliance detected. The `text-mode` layer is likely binding `t` globally. The standard requires tests to be under `SPC m t` (leader) or `, t` (local leader)."*

---

## Step 2: The Strategy (Override or Move?)

You know Layer A is stealing the key from Layer B.
We must decide: Do we use `spacemacs/set-leader-keys-for-major-mode` (specific) or change the loading order?

**Your Task:**
Stay with **Kallista** (or ask **Bob**).

> **Prompt:** "Should we move the Python test binding to `SPC m T` or force-override the shadowing binding?"

**Result:**
*"Override is acceptable if it restores the specific Major-Mode workflow. The 'Core Mandate' prioritizes local precision over global generic commands."*

---

## Step 3: The Fix (Using the Correct Macros)

Now we code. Common mistake: Using `(define-key ...)` which often breaks Evil states or the leader key map.

**Your Task:**
Switch to **Spacky**.

> **Command:** `/spacky`
> **Prompt:** "Fix the keybinding conflict in `python/packages.el`.
> Bind `python-test-run` explicitly to `SPC m t` (and `, t`) for `python-mode` users. Use the correct Æmacs macros to ensure it works for both Vim and Emacs styles."

**Result:**
Spacky will use the **Deterministic Reasoning Block** (Check: Holy vs. Evil Mode?) and output the code:

```elisp
;; Correct Æmacs way
(spacemacs/set-leader-keys-for-major-mode 'python-mode
  "t" 'python-test-run)
```

---

## 🎉 Summary

You have:
1.  Respected the hierarchy (Major Mode > Global Mode).
2.  Upheld "Urban Planning" rules (**Kallista**).
3.  Used the correct macros (**Spacky**) so Leader Keys (`SPC`) and Local Leader (`,`) work correctly.
