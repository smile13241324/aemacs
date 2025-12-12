# Tutorial 12: The "4D" Code Review

You have received a Pull Request (or written code yourself). Before merging, it must pass the "Gauntlet" of our specialized reviewers.

**Goal:** Perform a deep-dive review checking for Logic, Security, and Style.
**Time:** approx. 15 minutes.
**Prerequisite:** CLI agents installed via `sync-agents.py`.

---

## 🎭 Your AI Crew for this Job

1.  **Marjin (Refactorer):** Checks for code smell, complexity, and "decadence" (bad patterns).
2.  **Skeek (Security):** The paranoid rat-man. He hunts for bugs, logic errors, and security vulnerabilities ("cracks").
3.  **G.O.L.E.M. (Style):** The strictly bureaucratic guardian. He checks docstrings, headers, and naming conventions.

---

## Step 1: The General Health Check (Marjin)

**Scenario:** You have a buffer with new code (or a diff).

**Your Task:**
Use **Marjin**.

> **Command:** `/marjin`
> **Prompt:** "I have a new function here: `[PASTE CODE]`.
> Analyze it. Is it 'clean'? Does it use modern Elisp conventions (seq, pcase)? Or is it... 'untidy'?"

**Result:**
Marjin will sigh and tell you if you used `cl-loop` where a `seq-map` would be cleaner.

---

## Step 2: The Security Audit (Skeek)

Now we look for the dangerous stuff.

**Your Task:**
Switch to **Skeek**.

> **Command:** `/skeek`
> **Prompt:** "Sniff this code! Find the 'rot-holes'!
> 1. Are there input sanitization issues?
> 2. Are we using `eval` or `shell-command` dangerously?
> 3. Are there logic gaps where `nil` could crash it?"

**Result:**
Skeek gets excited about flaws: *"Yes-yes! A crack! You accept string argument but do not check if empty! The Man-thing's code will crash-burn!"*

---

## Step 3: The Bureaucracy (G.O.L.E.M.)

Finally, before merging, it must look professional.

**Your Task:**
Switch to **G.O.L.E.M.**.

> **Command:** `/golem`
> **Prompt:** "Review this code for Statutory Compliance.
> 1. Are docstrings present and imperative?
> 2. Are variable names compliant?
> 3. Is the indentation correct?"

**Result:**
*"Grind... Function `my-func`... docstring starts with 'Returns'... Violation. Must start with 'Return'. Compliance: 85%."*

---

## 🎉 Summary

You have passed the Gauntlet:
1.  **Clean Code:** Validated by Marjin.
2.  **Secure Code:** Sniffed by Skeek.
3.  **Compliant Code:** Stamped by G.O.L.E.M.
