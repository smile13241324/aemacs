# Tutorial 11: Professional Git Workflow

A clean Git history is as important as clean code. This tutorial shows you how to use AI to generate standardized commit messages and update the changelog.

**Goal:** Write a commit message that adheres to the "Tim Pope Standard" (Imperative, 50/72 wrapping) and update `CHANGELOG.md`.
**Time:** approx. 10 minutes.
**Prerequisite:** CLI agents installed via `sync-agents.py`.

---

## 🎭 Your AI Crew for this Job

1.  **G.O.L.E.M. (Style Guardian):** Enforces the strict commit message rules defined in `profile_doc.md`.
2.  **Griznak (Release Manager):** Manages the `CHANGELOG.md` and tracks changes for the next release.

---

## Step 1: The Draft (From Diff to Message)

**Scenario:** You have modified `ai/sync-agents.py` to support path detection. You are tired and don't know how to summarize it.

**Your Task:**
Use **G.O.L.E.M.**.

> **Command:** `/golem`
> **Prompt:** "I have modified `sync-agents.py`.
> Changes:
> 1. It now finds `coding_ai.md` relative to the script location.
> 2. It now puts profiles directly in `ai/` instead of `ai/profiles/`.
> Write a strict commit message for this."

**Result:**
G.O.L.E.M. will generate a compliant message:

```git
Add path detection to agent sync script

- Update script to locate `coding_ai.md` via `__file__` path
- Flatten profile directory structure to `ai/`
- Ensure script runs correctly from project root

This prevents FileNotFoundError when running outside the ai/ directory.
```

*Notice: He used "Add" (Imperative), not "Added". He wrapped the body text.*

---

## Step 2: The Audit (Fixing Bad Habits)

**Scenario:** You accidentally wrote a lazy message: *"Fixed the bug with the paths and updated python script."*

**Your Task:**
Stay with **G.O.L.E.M.**

> **Prompt:** "Critique this commit message: 'Fixed the bug with the paths and updated python script.'
> Rewrite it if it violates the Statutes."

**Result:**
*"Grind... Violation detected. Subject line uses Past Tense ('Fixed'). Subject is vague ('the bug').*
*Correction:*
`Fix path resolution in python sync script`"

---

## Step 3: The Changelog (Public Announcement)

If the change is visible to the user (e.g., a new feature), it belongs in `CHANGELOG.md`.

**Your Task:**
Switch to **Griznak**.

> **Command:** `/griznak`
> **Prompt:** "Create a one-line entry for `CHANGELOG.md` under the 'Unreleased' section for this change.
> Format: `[Layer/File] Description (PR#)`"

**Result:**
Griznak (likely screaming for coffee) delivers:

```markdown
- [AI] Make agent sync script executable from project root (#123)
```

---

## Step 4: Automating with Æmacs (The "Pro" Workflow)

You don't need to prompt manually every time. We have integrated G.O.L.E.M. directly into the `github-copilot` layer.

**Configuration:**
Enable the **G.O.L.E.M. mode** in your `.spacemacs` configuration layers list:

```elisp
(github-copilot :variables
                github-copilot-enable-commit-messages 'golem)
```

**Usage:**
1.  Open Magit Status (`SPC g s`).
2.  Stage your changes (`s`).
3.  Start the commit (`c c`).
4.  **Watch Magic:** G.O.L.E.M. will automatically analyze the diff and insert a compliant message into the buffer.

---

## 🎉 Summary

You have:
1.  Generated a professional commit message (**G.O.L.E.M.**).
2.  Learned the "Imperative Mood" rule.
3.  Updated the changelog with the correct role (**Griznak**).
4.  **Automated the workflow** via layer configuration.
