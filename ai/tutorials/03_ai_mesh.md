# Tutorial 03: The AI Mesh (Neural Integration)

Æmacs is an "AI Native" editor. We don't just call APIs; we run a local Multi-Agent System (MAS). This tutorial shows you how to script the brain.

**Goal:** Create a custom Python script that analyzes your code history using **Nagah**.
**Time:** approx. 20 minutes.
**Prerequisite:** `uv`, `python 3.12`.

---

## 🎭 Your AI Crew for this Job

1.  **Nagah (The Serpent):** The Python Specialist. She writes the glue code, handles dataframes, and manages the context window.
2.  **Kael'Thas (Project Owner):** Defines the permissions (what can the AI read?).

---

## Step 1: The Scripting Layer (Python Glue)

**Scenario:** You want an AI agent that scans your git history and summarizes your "coding mood" (e.g., are you committing angry messages?).

**Your Task:**
Use **Nagah**.

> **Command:** `/nagah`
> **Prompt:** "I want a Python script for the AI Mesh.
> 1. Use `gitpython` to read the last 50 commit messages.
> 2. Use `textblob` or a local LLM to analyze sentiment.
> 3. Return a JSON object with the 'Mood Score'."

**Result:**
Nagah glides through the implementation:

```python
import git
from textblob import TextBlob
from typing import Dict

def analyze_mood(repo_path: str) -> Dict[str, float]:
    repo = git.Repo(repo_path)
    commits = list(repo.iter_commits('master', max_count=50))

    polarity_sum = 0.0
    for commit in commits:
        blob = TextBlob(commit.message)
        polarity_sum += blob.sentiment.polarity

    return {"average_mood": polarity_sum / len(commits)}
```

---

## Step 2: Integration with Core (The Bridge)

How does Rust call this Python script? Via the `pyo3` bridge defined in the Core.

**Your Task:**
Switch to **Kairon**.

> **Command:** `/kairon`
> **Prompt:** "How do I register this Python function as a 'Capability' in the Rust Core?
> I want to trigger it via `SPC a i m` (AI Mood)."

**Result:**
Kairon shows you the `capabilities.toml` registration:

```toml
[capability.mood_tracker]
handler = "python.mood_analyzer"
trigger = "user_command"
permissions = ["read_git"]
```

---

## 🎉 Summary

You have:
1.  Scripted logic in Python (**Nagah**).
2.  Bridged it to the Rust Core (**Kairon**).
3.  Created a new AI Feature.
