# Tutorial 08: UI, Icons & Theming

Æmacs is famous for its aesthetics. This tutorial shows you how to design new icons (SVG), define faces (colors), and adapt the modeline without breaking terminal compatibility.

**Goal:** Design and implement a new UI element (e.g., a status icon).
**Time:** approx. 30 minutes.
**Prerequisite:** CLI agents installed via `sync-agents.py`.

---

## 🎭 Your AI Crew for this Job

1.  **Magos Pixelis (UI Designer):** The Architect. He drafts the concept, adheres to the "Sacred 8-Pixel Grid," and decides how it looks in GUI vs. Terminal.
2.  **Bzzrts (UI Implementor):** The Medium. He receives the vision from the Magos and transmutes it into perfect SVG code or `defface` definitions.

---

## Step 1: The Concept (The Vision)

**Scenario:** We want a "Privacy Mode" icon in the modeline.

**Your Task:**
Use **Magos Pixelis**.

> **Command:** `/magos`
> **Prompt:** "I need a concept for a 'Privacy Mode' icon in the modeline.
> 1. It must follow the 'Sacred 8-Pixel Grid'.
> 2. What should it look like in GUI (SVG)?
> 3. What is the text-fallback for Terminal (`-nw`)?"

**Result:**
Magos will (fanatically) answer: *"The Grid demands purity!
**GUI:** A stylized eye... *closed*. Or a shield. Minimalist lines. 16x16 canvas.
**Terminal:** The rune `[P]` or `(Ø)`. Simple. Brutal. Functional."*

---

## Step 2: SVG Creation (The Alchemy)

We need the SVG code. Important: It must use `currentColor` so it automatically adapts to light and dark themes.

**Your Task:**
Switch to **Bzzrts**.

> **Command:** `/bzzrts`
> **Prompt:** "Receive the vision of the 'Privacy Shield' icon.
> Generate the Elisp code to define this SVG image using `create-image`.
> **CRITICAL:** Use `fill='currentColor'` and ensure a valid `viewBox`."

**Result:**
Bzzrts sends you the code containing the path data:

```elisp
(defconst my-privacy-icon
  (create-image "<svg viewBox='0 0 16 16' fill='currentColor'>...</svg>"
                'svg t :ascent 'center))
```

---

## Step 3: The Colors (Defining Faces)

An icon often needs color (e.g., Green for "Active"). We must NOT hardcode Hex codes (`#FF0000`)! We must use `defface`.

**Your Task:**
Stay with **Bzzrts**.

> **Prompt:** "Define a face `my-privacy-face`.
> It should inherit from `success` (green) by default, but be bold.
> Explain why inheriting is better than hardcoding colors."

**Result:**
The code for `defface`, ensuring it looks good in *any* user theme.

---

## Step 4: The Integration (The Modeline Segment)

Now we assemble it all into the modeline.

**Your Task:**
Ask **Bzzrts** for the logic.

> **Prompt:** "Write a function `my-privacy-segment` that returns:
> - The SVG icon (propertized with our face) if in GUI.
> - The text `[P]` if in Terminal.
> Use `(display-graphic-p)` to check."

**Result:**
The finished Elisp snippet that ensures your feature won't crash when you start Æmacs via SSH (Terminal).

```elisp
(defun my-privacy-segment ()
  (if (display-graphic-p)
      (propertize " " 'display my-privacy-icon 'face 'my-privacy-face)
    (propertize "[P]" 'face 'my-privacy-face)))
```

---

## 🎉 Summary

You have:
1.  Created a design concept (**Magos**).
2.  Built a responsive SVG (**Bzzrts**).
3.  Defined theme-compatible colors (**Bzzrts**).
4.  Secured terminal support (Profile Rule).
