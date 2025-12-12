# Tutorial 10: Testing & Quality Assurance (Buttercup)

Code that isn't tested is broken. This tutorial shows you how to generate modern BDD-style tests (Behavior Driven Development) using the **Buttercup** framework.

**Goal:** Write a `tests.el` file that verifies logic, keybindings, and user behavior using `describe` and `it` blocks.
**Time:** approx. 20 minutes.
**Prerequisite:** CLI agents installed via `sync-agents.py`.

---

## 🎭 Your AI Crew for this Job

1.  **Don Testote (QA Engineer):** The Knight of Quality. He specializes in Buttercup specs, spies, and mocks. He **REQUIRES** `profile_elisp_testing.md`.
2.  **Spacky (Coder):** Helps with running the tests and setting up the CI environment.

---

## Step 1: Logic Tests (The Spec)

**Scenario:** You have a function `bisexual-pride/next-color` and want to ensure the color cycle logic is correct.

**Your Task:**
Use **Don Testote**.

> **Command:** `/don_testote`
> **Prompt:** "I need a Buttercup spec for the function `bisexual-pride/next-color`.
> Logic: It should cycle (Pink -> Purple -> Blue -> Pink).
> Write a `describe` block with multiple `it` clauses asserting the return values."

**Result:**
Don Testote writes a clean BDD test:

```elisp
(describe "The bisexual-pride/next-color function"
  (it "cycles from pink to purple"
    (expect (bisexual-pride/next-color 'pink) :to-be 'purple))

  (it "cycles from blue back to pink"
    (expect (bisexual-pride/next-color 'blue) :to-be 'pink)))
```

---

## Step 2: Keybinding Verification (Compliance)

Does `SPC o b` actually point to the right command?

**Your Task:**
Stay with **Don Testote**.

> **Prompt:** "Generate a test to verify that `SPC o b` is correctly bound to `bisexual-pride/toggle` in the `spacemacs-default-map`.
> Use `lookup-key` inside an `it` block."

**Result:**
A test asserting your configuration:

```elisp
(describe "Keybindings"
  (it "binds SPC o b to the toggle command"
    (let ((cmd (lookup-key spacemacs-default-map (kbd "SPC o b"))))
      (expect cmd :to-be 'bisexual-pride/toggle))))
```

---

## Step 3: Behavioral Tests (Integration)

Now the "Real World" test. We simulate a user typing and checking the buffer state.

**Your Task:**
Stay with **Don Testote**.

> **Prompt:** "I need an integration test.
> Scenario:
> 1. Create a temp buffer with `bi-mode` active.
> 2. Insert text 'Hello'.
> 3. Run `bisexual-pride/colorize-buffer`.
> 4. Expect the first character to have the face `bisexual-pride-pink-face`.
> Use `with-temp-buffer` inside the `it` block."

**Result:**
Don Testote writes a robust scenario:

```elisp
(describe "Buffer Colorization"
  (it "applies the correct face to the first character"
    (with-temp-buffer
      (bi-mode)
      (insert "Hello")
      (bisexual-pride/colorize-buffer)
      (expect (get-text-property 1 'face) :to-be 'bisexual-pride-pink-face))))
```

---

## Step 4: Running the Tests

**Your Task:**
Ask **Don Testote** how to execute this.

> **Prompt:** "How do I run these Buttercup tests interactively?"

**Result:**
*"Open the file and run `M-x buttercup-run-at-point` for a single test, or `SPC m t b` (if configured) to run the whole suite."*

---

## 🎉 Summary

You have:
1.  Defined clear Specs (**Buttercup**).
2.  Verified Logic & Config (**Don Testote**).
3.  Simulated User Actions (**Integration**).
