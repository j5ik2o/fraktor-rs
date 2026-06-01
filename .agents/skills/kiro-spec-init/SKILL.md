---
name: kiro-spec-init
description: Initialize a new specification with detailed project description
---


# Spec Initialization

<instructions>
## Core Task
Resolve or generate a unique feature name from the project description ($ARGUMENTS) and initialize the specification structure.

## Execution Steps
1. **Resolve Feature Name**: If $ARGUMENTS names an existing `.kiro/specs/{feature-name}/` directory, use that directory name. Otherwise generate the feature name from the project description.
2. **Check for Brief**: After resolving the feature name, read `.kiro/specs/{feature-name}/brief.md` if it exists. If no brief exists at the resolved name but a brief-only spec directory clearly matches the request, switch to that directory instead of creating a duplicate spec directory. If multiple brief-only directories could match, ask the user to choose before continuing. The brief contains problem, approach, scope, and constraints from the discovery session. Use this to pre-fill the project description and skip clarification questions that the brief already answers.
3. **Clarify Intent**: The Project Description in requirements.md must contain three elements: (a) who has the problem, (b) current situation, (c) what should change. If a brief.md exists and covers these, skip to step 4. Otherwise, ask the user to clarify before proceeding. Ask as many questions as needed; do not fill in gaps with your own assumptions.
4. **Check Uniqueness**: Verify `.kiro/specs/` for naming conflicts. If step 1 selected an existing `.kiro/specs/{feature-name}/` directory, use it in place and do not generate a suffixed sibling. If the directory already exists with only `brief.md` (no `spec.json`), use that directory (discovery created it).
5. **Create Directory**: `.kiro/specs/[feature-name]/` (skip if already exists from discovery)
6. **Initialize Files Using Templates**:
   - Read `.kiro/settings/templates/specs/init.json`
   - Read `.kiro/settings/templates/specs/requirements-init.md`
   - Replace placeholders:
     - `{{FEATURE_NAME}}` → generated feature name
     - `{{TIMESTAMP}}` → current ISO 8601 timestamp
     - `{{PROJECT_DESCRIPTION}}` → from brief.md if available, otherwise $ARGUMENTS
     - `ja` → language code (detect from user's input language, default to `en`)
   - Write only missing initialization files to the spec directory
   - If `spec.json` or `requirements.md` already exists, preserve the existing file and do not overwrite it with a template stub
   - If both `spec.json` and `requirements.md` already exist, stop and report that the spec is already initialized; point to the next appropriate phase instead of rewriting files

## Important Constraints
- Do NOT generate requirements, design, or tasks. This skill only creates spec.json and requirements.md.
- Do NOT overwrite an existing initialized spec. Existing `spec.json` and `requirements.md` are source of truth for resume flows.
</instructions>

## Output Description
Provide output in the language specified in `spec.json` with the following structure:

1. **Generated Feature Name**: `feature-name` format with 1-2 sentence rationale
2. **Project Summary**: Brief summary (1 sentence)
3. **Created Files**: Bullet list with full paths
4. **Next Step**: Command block showing `$kiro-spec-requirements <feature-name>`

**Format Requirements**:
- Use Markdown headings (##, ###)
- Wrap commands in code blocks
- Keep total output concise (under 250 words)
- Use clear, professional language per `spec.json.language`

## Safety & Fallback
- **Ambiguous Feature Name**: If feature name generation is unclear, propose 2-3 options and ask user to select
- **Template Missing**: If template files don't exist in `.kiro/settings/templates/specs/`, report error with specific missing file path and suggest checking repository setup
- **Directory Conflict**: If the user named an existing `.kiro/specs/{feature-name}/`, use the existing directory and follow the overwrite-prevention rules above. If a generated name collides with an unrelated initialized spec, stop and ask for a distinct feature name; do not create a suffixed sibling automatically.
- **Write Failure**: Report error with specific path and suggest checking permissions or disk space
