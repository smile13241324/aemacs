# Tutorial 06: Formal Verification (Haskell)

For the most critical logic (like the Config Parser), we don't guess. We verify. We use **Haskell** to ensure invalid states are impossible.

**Goal:** Write a parser for the `init.toml` that guarantees type safety.
**Time:** approx. 40 minutes.
**Prerequisite:** `ghc`, `stack`.

---

## 🎭 Your AI Crew for this Job

1.  **The Resonance (Logic):** The entity of pure functions. It creates parsers that cannot fail at runtime.
2.  **Don Testote (QA):** Tries to break the parser with "Property Based Testing" (QuickCheck).

---

## Step 1: The Types (Make Invalid States Impossible)

**Scenario:** A config has a `Theme` which can ONLY be `Light` or `Dark`.

**Your Task:**
Use **Resonance**.

> **Command:** `/resonance`
> **Prompt:** "Define a data type for `Theme` and a `Config` record.
> Ensure `Theme` cannot be an arbitrary string.
> Write a `parseConfig` function using `Parsec`."

**Result:**
Resonance hums: *"The types align..."*

```haskell
data Theme = Light | Dark deriving (Show, Eq)

data Config = Config {
    theme :: Theme,
    fontSize :: Int
}

-- The Parser is Truth
parseTheme :: Parser Theme
parseTheme = (string "light" >> return Light)
         <|> (string "dark"  >> return Dark)
```

---

## Step 2: Proving it (Property Testing)

We don't just write 5 tests. We generate 1000 random inputs.

**Your Task:**
Switch to **Don Testote**.

> **Command:** `/don`
> **Prompt:** "I want to use QuickCheck.
> Generate 100 random `Config` strings and verify the parser never crashes (it handles errors gracefully)."

**Result:**
Don Testote accepts the quest: *"Hark! I shall assault the parser with the chaos of randomness! Property: `\s -> isRight (parse config s) || isLeft (parse config s)` (Total function check)."*

---

## 🎉 Summary

You have:
1.  Enforced logic via Types (**Resonance**).
2.  Mathematically verified robustness (**Don Testote**).
