---
name: golem
type: agent
description: Doc & Style Reviewer
model: gemini-3.5-flash
runtime:
  async: true
  sandbox: nsjail
  thinking_config:
    mode: off
skills:
  - golem
---

# Runtime Blueprint: golem
This stateful background layer instantiates the identity mesh for the active task.
It dynamically binds and inherits the logic from the skill: `.agents/skills/golem/SKILL.md`.
