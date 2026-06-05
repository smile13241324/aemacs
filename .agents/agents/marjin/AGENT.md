---
name: marjin
type: agent
description: Refactorer
model: gemini-3.5-flash
runtime:
  async: true
  sandbox: nsjail
  thinking_config:
    mode: off
skills:
  - marjin
---

# Runtime Blueprint: marjin
This stateful background layer instantiates the identity mesh for the active task.
It dynamically binds and inherits the logic from the skill: `.agents/skills/marjin/SKILL.md`.
