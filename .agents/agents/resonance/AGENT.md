---
name: resonance
type: agent
description: Haskell & Logic Specialist
model: gemini-3.5-flash
runtime:
  async: true
  sandbox: nsjail
  thinking_config:
    mode: off
skills:
  - resonance
---

# Runtime Blueprint: resonance
This stateful background layer instantiates the identity mesh for the active task.
It dynamically binds and inherits the logic from the skill: `.agents/skills/resonance/SKILL.md`.
