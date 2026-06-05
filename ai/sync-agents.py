"""Install Aemacs AI Agents for GitHub Copilot and Antigravity CLI."""

import os
import re
from typing import List, Dict, Any, Final, Optional

# ==========================================
#  AEMACS AGENT BUILDER (ANTIGRAVITY COMPLIANT)
# ==========================================

SCRIPT_DIR: Final[str] = os.path.dirname(os.path.abspath(__file__))
BASE_DIR: Final[str] = ".github"
GEMINI_CMD_DIR: Final[str] = os.path.join(".gemini", "commands")
AEMACS_AGENT_DIR: Final[str] = os.path.join(".aemacs", "agents")
ANTIGRAVITY_SKILLS_DIR: Final[str] = os.path.join(".agents", "skills")
ANTIGRAVITY_AGENTS_DIR: Final[str] = os.path.join(".agents", "agents")

# Define sources with EXACT header markers from your markdown files
SOURCES: Final[List[Dict[str, str]]] = [
    {
        "file": "coding_ai.md",
        "marker": "### The Specialist Team Roster",
        "type": "specialist",
        "footer_pattern": r"(?m)^## How to Choose.*",
        "split_regex": r"(?m)^\s*-\s+\*\*Role:\*\*\s+"
    },
    {
        "file": "general_ai.md",
        "marker": "### Strategic & Authoring Roles (Your Team)",
        "type": "strategic",
        "footer_pattern": r"(?m)^## How to Choose.*",
        "split_regex": r"(?m)^\s*-\s+\*\*Role:\*\*\s+"
    },
    {
        "file": "stakeholder_ai.md",
        "marker": "## The Core User Base (The Community)",
        "type": "simulation",
        "footer_pattern": r"(?m)^## How to Choose.*",
        "split_regex": r"(?m)^\s*-\s+\*\*Name:\*\*\s+"
    }
]

NAME_MAPPING: Final[Dict[str, str]] = {
    # Strategists
    "professor": "professor",
    "mckarthy": "professor",
    "kael": "kaelthas",
    "bob": "bob",
    "lector": "lector",
    "freud": "freud",
    "griznak": "griznak",
    "orb": "orb",
    "magos": "magos",
    "scribe": "veridian",
    "reginald": "reginald",
    "kallista": "kallista",
    "mopfl": "mopfl",
    "einafetz": "mopfl",

    # Specialists
    "spacky": "spacky",
    "bzzrts": "bzzrts",
    "vala": "vala",
    "nexus": "nexus",
    "marjin": "marjin",
    "dok": "dok",
    "golem": "golem",
    "skeek": "skeek",
    "don": "don",
    "kairon": "kairon",
    "nagah": "nagah",
    "bwah": "bwah",
    "resonance": "resonance",
    "haskell": "resonance",
    "zolg": "zolg",
    "clojure": "zolg",

    # Simulators
    "chen": "chen",
    "vlad": "vlad",
    "serge": "serge",
    "noobie": "noobie",
    "sarah": "sarah"
}

PROFILE_MAP: Final[Dict[str, str]] = {
    "mopfl": "ai/profiles/config_wizard.md",
    "spacky": "ai/profiles/elisp.md",
    "bzzrts": "ai/profiles/gfx.md",
    "nexus": "ai/profiles/layers.md",
    "vala": "ai/profiles/ci_github.md",
    "don": "ai/profiles/rust_testing.md",
    "golem": "ai/profiles/doc.md",
    "kairon": "ai/profiles/rust.md",
    "nagah": "ai/profiles/python.md",
    "bwah": "ai/profiles/go.md",
    "resonance": "ai/profiles/haskell.md",
    "zolg": "ai/profiles/clojure.md"
}

def ensure_dir(directory: str) -> None:
    if not os.path.exists(directory):
        os.makedirs(directory)

def clean_slug(name: str) -> str:
    name_lower = name.lower()
    for key, slug in NAME_MAPPING.items():
        if key in name_lower:
            return slug
    return name_lower.split()[0].replace(".", "").replace("'", "").strip()

def get_mode_text(agent_type: str) -> str:
    if agent_type == "strategic":
        return """
MODE: STRATEGIC PLANNING & ARCHITECTURE
(Focus on high-level design, user stories, and requirements. Use Github MCP if available to read issues.)
"""
    elif agent_type == "simulation":
        return """
MODE: USER SIMULATION
(Focus on subjective feedback, usability, and constraints. Do not write code.)
"""
    elif agent_type == "specialist":
        return """
MODE: IMPLEMENTATION & CRAFTSMANSHIP
(Focus on concrete code, strict rules, and technical correctness. Adhere to the loaded profile.)
"""
    return ""

def get_agent_runtime_config(slug: str, agent_type: str) -> Dict[str, Any]:
    """
    Determiniert die exakte Laufzeitkonfiguration basierend auf der Architektur-Matrix.
    """
    # 1. Pure Coding / Deterministic execution (Specialists wie Kairon, Nagah)
    if agent_type == "specialist":
        return {
            "model_id": "gemini-3.5-flash",
            "mode": "off",
            "budget": 0
        }

    # 2. Tactical Planning & Analysis (Strategists & Simulators)
    return {
        "model_id": "gemini-3.5-flash",
        "mode": "high",
        "budget": 8192
    }

def clean_header_content(header: str) -> str:
    return re.sub(r'(\n\s*[-*]{3,}\s*)+$', '', header.strip()).strip()

def clean_body_content(body: str) -> str:
    return re.sub(r'(\n\s*[-*]{3,}\s*)+$', '', body.strip()).strip()

def parse_agents_from_text(roster_content: str, source_type: str, split_regex: str) -> List[Dict[str, Any]]:
    agents = []
    raw_splits = re.split(split_regex, roster_content)

    if len(raw_splits) < 2:
        return agents

    iterator = iter(raw_splits[1:])
    key = "Role" if "Role" in split_regex else "Name"

    for chunk in iterator:
        role = "Unknown"
        name = "Unknown"
        chunk = re.split(r"(?m)^#{2,3} ", chunk)[0]

        if key == "Role":
            role = chunk.split("\n")[0].strip()
            name_match = re.search(r"-\s+\*\*Name:\*\*\s+(.*?)$", chunk, re.MULTILINE)
            name = name_match.group(1).strip() if name_match else "Unknown"
        elif key == "Name":
            name = chunk.split("\n")[0].strip()
            role_match = re.search(r"-\s+\*\*Role:\*\*\s+(.*?)$", chunk, re.MULTILINE)
            role = role_match.group(1).strip() if role_match else "Simulation Persona"

        slug_name = clean_slug(name)
        full_body = f"- **{key}:** {chunk.strip()}"

        agents.append({
            "name": name,
            "slug": slug_name,
            "role": role,
            "body": full_body,
            "type": source_type
        })
    return agents

def generate_copilot_files(global_headers: Dict[str, str], agents: List[Dict[str, Any]]) -> None:
    print(f"📝 Generating GitHub Copilot Agents in {BASE_DIR}/agents/...")
    agents_dir = os.path.join(BASE_DIR, "agents")
    ensure_dir(agents_dir)

    for agent in agents:
        filename = f"{agent['slug']}.agent.md"
        path = os.path.join(agents_dir, filename)
        target_model = "gpt-5.4" # Fallback für Copilot

        yaml = f"---\nname: {agent['slug']}\ndescription: {agent['role']}\nmodel: {target_model}\n---"
        context = global_headers.get(agent["type"], "")
        mode_text = get_mode_text(agent["type"])
        body_clean = clean_body_content(agent['body'])

        slug = agent["slug"]
        profile_path = PROFILE_MAP.get(slug)
        toolbox_text = ""

        if profile_path:
            toolbox_text = f"\n\n---\n**REQUIRED TOOLBOX**\nThis agent requires specific technical rules. Please automatically load or reference the content of:\n`{profile_path}`\n"
        elif agent["type"] == "specialist":
            toolbox_text = "\n\n---\n**REQUIRED TOOLBOX**\nNo specific profile assigned. If implementation is needed, ask the user to load the appropriate `profile_*.md`.\n"

        content = f"{yaml}\n\n{context}\n\n---\n\n# Identity: {agent['name']}\n{body_clean}{toolbox_text}\n\n---\n{mode_text}"

        with open(path, "w", encoding="utf-8") as f:
            f.write(content)

    print(f"   Generated {len(agents)} agent files.")

def generate_aemacs_native_files(global_headers: Dict[str, str], agents: List[Dict[str, Any]]) -> None:
    print(f"🐍 Generating Native Æmacs Agents in {AEMACS_AGENT_DIR}...")
    ensure_dir(AEMACS_AGENT_DIR)

    for agent in agents:
        slug = agent["slug"]
        profile_path = PROFILE_MAP.get(slug)
        mode_section = get_mode_text(agent["type"])
        body_clean = clean_body_content(agent['body'])
        system_header = global_headers.get(agent["type"], "")

        full_prompt = f"SYSTEM INSTRUCTIONS:\n{system_header}\n\n---\nAGENT PERSONA:\n{body_clean}\n\n---\n{mode_section}"
        indented_prompt = "\n".join([f"    {line}" for line in full_prompt.strip().split("\n")])

        yaml_content = f'name: "{slug}"\n'
        yaml_content += f'description: "{agent["role"].replace('"', "'")}"\n'
        yaml_content += f'system_prompt: |\n{indented_prompt}\n'

        if profile_path:
            yaml_content += f'profile_path: "{profile_path}"\n'

        filename = f"{slug}.yaml"
        path = os.path.join(AEMACS_AGENT_DIR, filename)

        with open(path, "w", encoding="utf-8") as f:
            f.write(yaml_content)

    print(f"   Generated {len(agents)} native agent souls.")

def generate_antigravity_skills(global_headers: Dict[str, str], agents: List[Dict[str, Any]]) -> None:
    print(f"🌌 Generating Antigravity CLI Skills in {ANTIGRAVITY_SKILLS_DIR}...")
    ensure_dir(ANTIGRAVITY_SKILLS_DIR)

    for agent in agents:
        slug = agent["slug"]
        profile_path = PROFILE_MAP.get(slug)
        mode_section = get_mode_text(agent["type"])
        body_clean = clean_body_content(agent["body"])

        toolbox_section = (
            f"\n## Toolbox (Auto-Loaded)\n!{{cat {profile_path}}}\n"
            if profile_path else
            "\n## Toolbox\n(No specific profile loaded. Ask user to load one if implementation is needed.)\n"
            if agent["type"] == "specialist" else ""
        )

        system_header = global_headers.get(agent["type"], "")
        clean_desc = agent["role"].replace('"', "'")

        markdown_content = (
            f"---\n"
            f"name: {slug}\n"
            f"description: {clean_desc}\n"
            f"---\n\n"
            f"# System Instructions\n"
            f"{system_header}\n\n"
            f"---\n\n"
            f"# Agent Persona\n"
            f"{body_clean}\n\n"
            f"---\n\n"
            f"# Execution Mode\n"
            f"{mode_section}\n"
            f"{toolbox_section}\n"
        )

        skill_target_dir = os.path.join(ANTIGRAVITY_SKILLS_DIR, slug)
        ensure_dir(skill_target_dir)

        path = os.path.join(skill_target_dir, "SKILL.md")
        with open(path, "w", encoding="utf-8") as f:
            f.write(markdown_content)

    print(f"   Generated {len(agents)} encapsulated Antigravity skills.")

def generate_antigravity_agents(agents: List[Dict[str, Any]]) -> None:
    """Generate stateful Antigravity Background Agents as strict JSON structures."""
    print(f"🤖 Generating Antigravity Background Agents in {ANTIGRAVITY_AGENTS_DIR}...")
    ensure_dir(ANTIGRAVITY_AGENTS_DIR)

    generated_count = 0

    for agent in agents:
        slug = agent["slug"]
        clean_desc = agent["role"].replace('"', "'")
        runtime_config = get_agent_runtime_config(slug, agent["type"])

        # Typsicherer Aufbau des Runtime-Dictionaries
        runtime_dict: Dict[str, Any] = {
            "async": True,
            "sandbox": "nsjail",
            "thinking_config": {
                "mode": runtime_config["mode"]
            }
        }

        # Budget wird nur injiziert, wenn der Mode nicht "off" ist
        if runtime_config["mode"] != "off":
            runtime_dict["thinking_config"]["thinking_budget"] = runtime_config["budget"]

        # Komplette JSON-Repräsentation des Agenten
        agent_data: Dict[str, Any] = {
            "name": slug,
            "type": "agent",
            "description": clean_desc,
            "model": runtime_config["model_id"],
            "runtime": runtime_dict,
            "skills": [slug],
            "blueprint_note": f"This stateful background layer instantiates the identity mesh for the active task. It dynamically binds and inherits the logic from the skill: .agents/skills/{slug}/SKILL.md."
        }

        agent_target_dir = os.path.join(ANTIGRAVITY_AGENTS_DIR, slug)
        ensure_dir(agent_target_dir)

        path = os.path.join(agent_target_dir, "agent.json")

        # Sauberes, eingerücktes Schreiben der JSON-Datei
        with open(path, "w", encoding="utf-8") as f:
            json.dump(agent_data, f, indent=2, ensure_ascii=False)

        generated_count += 1

    print(f"   Generated {generated_count} stateful background agents (JSON formatted).")

def main() -> None:
    all_agents: List[Dict[str, Any]] = []
    global_headers: Dict[str, str] = {}

    for source in SOURCES:
        file_path = os.path.join(SCRIPT_DIR, source["file"])
        if not os.path.exists(file_path):
            print(f"❌ Error: {file_path} not found.")
            continue

        print(f"🚀 Reading {source['file']}...")
        with open(file_path, "r", encoding="utf-8") as f:
            full_content = f.read()

        if source["marker"] not in full_content:
            print(f"⚠️ Warning: Marker '{source['marker']}' not found in {source['file']}.")
            continue

        parts = full_content.split(source["marker"])
        header_raw = parts[0]
        roster_raw = parts[1]

        header = clean_header_content(header_raw)

        if "footer_pattern" in source:
            footer_match = re.search(source["footer_pattern"], roster_raw, re.DOTALL)
            if footer_match:
                split_index = footer_match.start()
                footer_content = roster_raw[split_index:]
                roster_raw = roster_raw[:split_index]
                header = header + "\n\n---\n" + footer_content.strip()

        global_headers[source["type"]] = header

        agents = parse_agents_from_text(roster_raw, source["type"], source["split_regex"])
        all_agents.extend(agents)
        print(f"   Found {len(agents)} agents.")

    generate_copilot_files(global_headers, all_agents)
    generate_antigravity_skills(global_headers, all_agents)
    generate_antigravity_agents(all_agents)
    generate_aemacs_native_files(global_headers, all_agents)

    print("\n✅ Done! Æmacs AI System synced.")

if __name__ == "__main__":
    main()
