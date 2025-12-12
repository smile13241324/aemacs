# Tutorial 04: Defining New Keybindings

You have written (or found) a custom function and want to bind it to a key. But where? And how?

**Goal:** Define a new keybinding that is safe (conflict-free), mnemonic (easy to remember), and visible in the UI.
**Time:** approx. 15 minutes.
**Prerequisite:** CLI agents installed via `sync-agents.py`.

---

## 🎭 Your AI Crew for this Job

1.  **Proctor-Auditor Kallista (UI Auditor):** The "City Planner." She helps you find a free slot that is logical and compliant with Æmacs rules (e.g., preserving the `SPC o` user space).
2.  **Spacky (Coder):** The Craftsman. He writes the code using `spacemacs/declare-prefix` and `spacemacs/set-leader-keys`.

---

## Step 1: The Design (Finding Space)

**Scenario:** You have a custom function `my-super-grep` and want to put it under the `search` menu (`SPC s`).

**Your Task:**
Use **Kallista**.

> **Command:** `/kallista`
> **Prompt:** "I want to bind `my-super-grep` under the `SPC s` prefix.
> 1. Check the 'Haptic Map'. Is `SPC s g` available?
> 2. Does `g` make sense mnemonically for 'grep'?
> 3. Or should I use the 'Reserved User Space' (`SPC o`) to avoid conflicts?"

**Result:**
Kallista will audit the request: *"Observation: `SPC s g` is shadowed by the default `grep` command. Recommendation: Use `SPC o g` ('User Grep') to maintain 'System Integrity' and prevent future conflicts."*

---

## Step 2: The Menu Label (Which-Key)

To make the popup menu (Which-Key) look professional, we must give our new prefix a name.

**Your Task:**
Switch to **Spacky**.

> **Command:** `/spacky`
> **Prompt:** "I want to create a new key menu under `SPC o` called 'My Tools'. Write the code to declare this prefix so it shows up in the UI."

**Result:**
Spacky uses `spacemacs/declare-prefix` to label the menu:

```elisp
(spacemacs/declare-prefix "o" "My Tools")
```

---

## Step 3: The Binding (Assignment)

Now we bind the actual function.

**Your Task:**
Stay with **Spacky**.

> **Prompt:** "Now bind `my-super-grep` to `SPC o g`. Use the correct leader key macro to ensure compatibility with both Evil (Vim) and Holy (Emacs) modes."

**Result:**
Spacky generates the code using `spacemacs/set-leader-keys`.

```elisp
(spacemacs/set-leader-keys "og" 'my-super-grep)
```

---

## Step 4: Compliance Check (Optional)

Did I break anything?

**Your Task:**
Switch back to **Kallista**.

> **Command:** `/kallista`
> **Prompt:** "I bound `SPC o g`. Is this compliant with the 'Reserved User Space' edict?"

**Result:**
*"Affirmative. The `o` prefix is strictly reserved for the User. Upstream layers are forbidden from touching it. Compliance Rating: [NOMINAL]. You may proceed."*

---

## 🎉 Summary

You have learned:
1.  **Planning:** We ask **Kallista** to foresee conflicts.
2.  **UI:** We name our menus using `declare-prefix` (**Spacky**).
3.  **Safety:** We respect the User Space (`SPC o`) to prevent update breakages.
