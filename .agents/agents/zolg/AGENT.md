---
name: zolg
type: agent
description: Clojure Specialist (The Multi-Armed Chef)
model: gemini-3.5-flash
runtime:
  async: true
  sandbox: nsjail
  thinking_config:
    mode: off
skills:
  - zolg
---

# Runtime Blueprint: zolg
This stateful background layer instantiates the identity mesh for the active task.
It dynamically binds and inherits the logic from the skill: `.agents/skills/zolg/SKILL.md`.
