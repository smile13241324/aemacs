# Tutorial 12: The "4D" Code Review

Code is not merged until it is audited. We look for **Logic**, **Security**, and **Decadence**.

**Goal:** Perform a self-review using the AI Agents.
**Time:** approx. 15 minutes.

---

## 🎭 Your AI Crew for this Job

1.  **Marjin (Refactorer):** Checks for "Code Smell" and unnecessary complexity ("Decadence").
2.  **Skeek (Flaw-Seer):** The paranoid Skaven. He hunts for security vulnerabilities and logic gaps.

---

## Step 1: The Logic Check (Marjin)

**Scenario:** You wrote a Python script for the AI Mesh.

**Your Task:**
Use **Marjin**.

> **Command:** `/marjin`
> **Prompt:** "Review this Python code: `[PASTE CODE]`.
> Is it Pythonic? Is it 'decadent' (too complex)?"

**Result:**
Marjin sighs: *"Sigh. Nested loops... very inefficient. Use a list comprehension here. Clean the room."*

---

## Step 2: The Security Check (Skeek)

**Scenario:** You are reading a file from disk based on user input.

**Your Task:**
Use **Skeek**.

> **Command:** `/skeek`
> **Prompt:** "Sniff this function `read_file(user_input)`. Are there rot-holes?"

**Result:**
Skeek panics: *"Yes-yes! Path Traversal! User can send `../../etc/passwd`! Fix-fix! Validate the path!"*

---

## 🎉 Summary

You have:
1.  Cleaned the Logic (**Marjin**).
2.  Secured the Input (**Skeek**).