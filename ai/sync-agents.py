"""Install Aemacs AI Agents for GitHub Copilot and Gemini CLI."""

import os
import re

# ==========================================
#  AEMACS AGENT BUILDER (V21 - MOPFL & REFORGED)
# ==========================================
# UPDATES:
# - Added Mopfl (Config Wizard) mapping
# - Updated Stakeholders (RMS -> Serge)
# - Adjusted Markers to match new file headers
# ==========================================

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
BASE_DIR = ".github"
GEMINI_CMD_DIR = os.path.join(".gemini", "commands")
AEMACS_AGENT_DIR = os.path.join(".aemacs", "agents")

# Define sources with EXACT header markers from your markdown files
SOURCES = [
    {
        "file": "coding_ai.md",
        "marker": "### The Specialist Team Roster",
        "type": "specialist",
        "footer_pattern": r"(?m)^## How to Choose.*",
        # Split only on Role to avoid splitting on nested Name fields
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
        # Stakeholders are defined by Name
        "split_regex": r"(?m)^\s*-\s+\*\*Name:\*\*\s+"
    }
]

NAME_MAPPING = {
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

# Maps Persona Slugs to their specific Profile Markdown file
PROFILE_MAP = {
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

def ensure_dir(directory):
    if not os.path.exists(directory):
        os.makedirs(directory)

def clean_slug(name):
    name_lower = name.lower()
    for key, slug in NAME_MAPPING.items():
        if key in name_lower:
            return slug
    return name_lower.split()[0].replace(".", "").replace("'", "").strip()

def get_mode_text(agent_type):
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

def get_model_id(agent_type):
    if agent_type == "specialist":
        return "gpt-5.1-codex"
    else:
        return "gpt-5.1"

def clean_header_content(header):
    cleaned = re.sub(r'(\n\s*[-*]{3,}\s*)+$', '', header.strip())
    return cleaned.strip()

def clean_body_content(body):
    cleaned = re.sub(r'(\n\s*[-*]{3,}\s*)+$', '', body.strip())
    return cleaned.strip()

def parse_agents_from_text(roster_content, source_type, split_regex):
    agents = []
    # Split using the specific regex for this file type
    raw_splits = re.split(split_regex, roster_content)

    if len(raw_splits) < 2:
        return agents

    iterator = iter(raw_splits[1:])
    key = "Role" if "Role" in split_regex else "Name"

    for chunk in iterator:
        role = "Unknown"
        name = "Unknown"

        # Clean trailing headers like "### " or "## "
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

def generate_copilot_files(global_headers, agents):
    print(f"📝 Generating GitHub Copilot Agents in {BASE_DIR}/agents/...")
    agents_dir = os.path.join(BASE_DIR, "agents")
    ensure_dir(agents_dir)

    for agent in agents:
        filename = f"{agent['slug']}.agent.md"
        path = os.path.join(agents_dir, filename)
        target_model = get_model_id(agent["type"])

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

def generate_gemini_commands(global_headers, agents):
    print(f"💎 Generating Gemini CLI Commands in {GEMINI_CMD_DIR}...")
    ensure_dir(GEMINI_CMD_DIR)

    for agent in agents:
        slug = agent["slug"]
        profile_path = PROFILE_MAP.get(slug)
        mode_section = get_mode_text(agent["type"])
        body_clean = clean_body_content(agent['body'])

        toolbox_section = ""
        if profile_path:
            toolbox_section = f"\nTOOLBOX (AUTO-LOADED):\n!{{cat {profile_path}}}\n"
        elif agent["type"] == "specialist":
             toolbox_section = "\nTOOLBOX:\n(No specific profile loaded. Ask user to load one if implementation is needed.)\n"

        system_header = global_headers.get(agent["type"], "")

        prompt_text = f"""
SYSTEM INSTRUCTIONS:
{system_header}

---
AGENT PERSONA:
{body_clean}

---
{mode_section}
{toolbox_section}
---
USER INPUT:
{{{{args}}}}
"""
        clean_desc = agent['role'].replace('"', "'")
        toml_content = f'description = "{clean_desc}"\n'
        toml_content += 'prompt = """' + prompt_text + '"""\n'

        filename = f"{slug}.toml"
        path = os.path.join(GEMINI_CMD_DIR, filename)

        with open(path, "w", encoding="utf-8") as f:
            f.write(toml_content)

    print(f"   Generated {len(agents)} commands.")

def generate_aemacs_native_files(global_headers: dict[str, str], agents: list[dict]) -> None:
    """Generate native Æmacs Agent YAML files in .aemacs/agents/."""
    print(f"🐍 Generating Native Æmacs Agents in {AEMACS_AGENT_DIR}...")
    ensure_dir(AEMACS_AGENT_DIR)

    for agent in agents:
        slug = agent["slug"]
        profile_path = PROFILE_MAP.get(slug)
        mode_section = get_mode_text(agent["type"])
        body_clean = clean_body_content(agent['body'])
        system_header = global_headers.get(agent["type"], "")

        # Assemble full system prompt without {{args}}
        full_prompt = f"SYSTEM INSTRUCTIONS:\n{system_header}\n\n---\nAGENT PERSONA:\n{body_clean}\n\n---\n{mode_section}"
        
        # Proper YAML block scalar indentation (2 spaces)
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

def main():
    all_agents = []
    global_headers = {}

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
            # Debugging hint
            print(f"   (Check if header in .md file matches: '{source['marker']}')")
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
    generate_gemini_commands(global_headers, all_agents)
    generate_aemacs_native_files(global_headers, all_agents)

    print("\n✅ Done! Æmacs AI System synced.")

if __name__ == "__main__":
    main()
