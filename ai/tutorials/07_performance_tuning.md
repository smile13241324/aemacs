# Tutorial 07: Performance Tuning (The Lag Killer)

Æmacs is powerful, but with many layers, startup time can suffer. This tutorial shows you how to find bottlenecks and make your Emacs fly again.

**Goal:** Analyze startup time, identify "Heavy Loaders," and optimize lazy-loading.
**Time:** approx. 20–30 minutes.
**Prerequisite:** CLI agents installed via `sync-agents.py`.

---

## 🎭 Your AI Crew for this Job

1.  **Nexus-7 (Profiler):** The Analyst. He reads the profiler report and tells you exactly which package is blocking the startup or main thread.
2.  **Marjin (Refactorer):** The Optimizer. He rewrites your code so packages only load when you actually need them (`:defer t`).

---

## Step 1: The Diagnosis (Who is braking?)

Before optimizing, we must measure. Æmacs has a built-in profiler.

**Your Task (in Æmacs):**
1.  Start the Profiler: `SPC h P s` (Start Profiler).
2.  Perform the slow action (e.g., startup or opening a file).
3.  Stop the Profiler: `SPC h P k` (Stop Profiler).
4.  Copy the report (Text).

**Your Task (with AI):**
Use **Nexus-7**.

> **Command:** `/nexus`
> **Prompt:** "I have a performance issue. Here is the profiler report: `[paste report]`.
> 1. Which function or package is consuming the most CPU/Memory?
> 2. Is this a 'Garbage Collection' issue or a 'Blocker' issue?"

**Result:**
Nexus-7 will analyze: *"Analysis complete. The package `super-heavy-mode` is taking 45% of startup time. It is being loaded eagerly in `init.el`."*

---

## Step 2: The Strategy (Enforce Lazy Loading)

We now know: `super-heavy-mode` loads too early. We must defer it.

**Your Task:**
Switch to **Marjin**.

> **Command:** `/marjin`
> **Prompt:** "Please refactor the configuration for `super-heavy-mode`.
> Currently it is: `(use-package super-heavy-mode :config (super-heavy-start))`.
> Change it to use `:defer t` and only load when I press `SPC o h`."

**Result:**
Marjin (sighing, but efficient) writes the optimized `use-package` block using `:commands` or `:bind`, ensuring the package only loads when you press the key.

```elisp
(use-package super-heavy-mode
  :defer t
  :commands (super-heavy-start)
  :init
  (spacemacs/set-leader-keys "oh" 'super-heavy-start))
```

---

## Step 3: The Hook Check (Hidden Brakes)

Often, slow functions hide in hooks (e.g., `prog-mode-hook`), running for *every* file you open.

**Your Task:**
Switch back to **Nexus-7**.

> **Command:** `/nexus`
> **Prompt:** "I want to add `flycheck-mode` to `prog-mode-hook`.
> Calculate the impact. Will this slow down opening large files?"

**Result:**
Nexus-7: *"Affirmative. Enabling global minor modes in generic hooks adds latency per buffer creation. Recommendation: Use specific hooks (e.g., `python-mode-hook`) instead of generic ones."*

---

## 🎉 Summary

You have:
1.  Measured instead of guessed (Profiler).
2.  Found the culprit (**Nexus-7**).
3.  Switched code to "Lazy" (**Marjin**).
