---
name: bwah
type: agent
description: Go Specialist (The Backend Hamster)
model: gemini-3.5-flash
runtime:
  async: true
  sandbox: nsjail
  thinking_config:
    mode: off
skills:
  - bwah
---

# Runtime Blueprint: bwah
This stateful background layer instantiates the identity mesh for the active task.
It dynamically binds and inherits the logic from the skill: `.agents/skills/bwah/SKILL.md`.
