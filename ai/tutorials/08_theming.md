# Tutorial 08: Theming (Vector Styling)

In Æmacs, a Theme is not just a list of hex codes. It is a **Struct**. We use a computed theming system (like Tailwind) to ensure consistency.

**Goal:** Create a "Bisexual Pride" theme using the Æmacs Palette Generator.
**Time:** approx. 15 minutes.
**Prerequisite:** `profile_gfx.md`.

---

## 🎭 Your AI Crew for this Job

1.  **Magos Pixelis:** Ensures the contrast ratio is compliant (WCAG AA).
2.  **Nagah (Python):** We use Python to *generate* the Rust theme structs from a JSON palette (because writing structs by hand is tedious).

---

## Step 1: The Palette (JSON)

**Scenario:** You have the hex codes: Pink (`#D60270`), Purple (`#9B4F96`), Blue (`#0038A8`).

**Your Task:**
Use **Nagah**.

> **Command:** `/nagah`
> **Prompt:** "I have these 3 base colors.
> Generate a `palette.json` that expands them into a full UI scale (Bg, Fg, Border, Surface).
> Use `colormath` to generate 5 shades of each."

**Result:**
Nagah weaves the colors:

```json
{
  "name": "BiPride",
  "base": {
    "pink": "#D60270",
    "purple": "#9B4F96",
    "blue": "#0038A8"
  },
  "generated": {
    "bg_primary": "#050010",
    "fg_primary": "#E0E0E0",
    "accent": "#D60270"
  }
}
```

---

## Step 2: The Audit (Contrast)

Is it readable?

**Your Task:**
Switch to **Magos Pixelis**.

> **Command:** `/magos`
> **Prompt:** "Audit this palette. Is the contrast between `bg_primary` and `fg_primary` sufficient for the Machine Spirit?"

**Result:**
Magos: *"Calculated Ratio: 15:1. [NOMINAL]. The text is legible. The Omnissiah approves."*

---

## Step 3: Compiling to Rust (The Build)

We don't parse JSON at runtime (slow). We compile it to Rust code.

**Your Task:**
Ask **Kairon**.

> **Command:** `/kairon`
> **Prompt:** "Run the `theme_compiler` macro to convert `palette.json` into `src/themes/bipride.rs`."

**Result:**
Kairon runs the build script and generates the zero-cost struct.

---

## 🎉 Summary

You have:
1.  Generated a palette algorithmically (**Nagah**).
2.  Verified accessibility (**Magos**).
3.  Compiled it for performance (**Kairon**).
