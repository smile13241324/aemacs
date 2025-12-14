# Tutorial 04: Python Scripting (The Glue)

Æmacs exposes a high-speed Python bridge via `PyO3`. This allows you to use the entire PyData ecosystem (Pandas, Numpy) directly within the editor.

**Goal:** Write a script that formats a CSV buffer using `pandas`.
**Time:** approx. 15 minutes.
**Prerequisite:** `uv` (Python Tooling).

---

## 🎭 Your AI Crew for this Job

1.  **Nagah (The Serpent):** She writes elegant, vectorized Python code. She hates loops.
2.  **Dr. Chen (Data Scientist):** He validates that the tool is actually useful for science.

---

## Step 1: The Requirement (Scientific Need)

**Scenario:** You have a messy CSV in a buffer. You want to align it and sort by column "Date".

**Your Task:**
Ask **Dr. Chen** (Simulation) first to refine the need.

> **Command:** `/chen`
> **Prompt:** "I'm building a 'CSV Tidier' for Æmacs. It uses Pandas. What features do you absolutely need?"

**Result:**
Dr. Chen: *"If it destroys my NaN values, I will riot. It must handle ISO8601 dates automatically. And it needs to be fast on 1GB files."*

---

## Step 2: The Script (Vectorized Logic)

**Your Task:**
Switch to **Nagah**.

> **Command:** `/nagah`
> **Prompt:** "Write a Python function `tidy_csv(content: str) -> str`.
> 1. Use `pandas` to read the CSV string.
> 2. Sort by 'Date'.
> 3. Return the formatted string (markdown table format).
> **Constraint:** Use vectorization, no for-loops!"

**Result:**
Nagah glides through the logic:

```python
import pandas as pd
from io import StringIO

def tidy_csv(content: str) -> str:
    df = pd.read_csv(StringIO(content))
    if 'Date' in df.columns:
        df['Date'] = pd.to_datetime(df['Date'])
        df = df.sort_values('Date')
    return df.to_markdown(index=False)
```

---

## Step 3: Binding the Command

Now we tell Æmacs to use this script.

**Your Task:**
Ask **Kairon** (The Core Bridge).

> **Command:** `/kairon`
> **Prompt:** "How do I bind this Python function to the command `tidy-buffer`?"

**Result:**
Kairon shows the registration hook in `capabilities.toml`.

---

## 🎉 Summary

You have:
1.  Validated needs (**Dr. Chen**).
2.  Written high-performance Python (**Nagah**).
3.  Integrated it seamlessly.
