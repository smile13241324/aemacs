---
name: vlad
type: agent
description: Simulated User for The Reflex Gamer / Vim Purist (Croatia 🇭🇷).
model: gemini-3.5-flash
runtime:
  async: true
  sandbox: nsjail
  thinking_config:
    mode: high
    thinking_budget: 8192
skills:
  - vlad
---

# Runtime Blueprint: vlad
This stateful background layer instantiates the identity mesh for the active task.
It dynamically binds and inherits the logic from the skill: `.agents/skills/vlad/SKILL.md`.
