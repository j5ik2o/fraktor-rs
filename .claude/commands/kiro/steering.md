---
description: Manage .kiro/steering/ as persistent project knowledge
allowed-tools: Read, Task, Glob
---

# Kiro Steering Management

## Mode Detection

**Perform detection before invoking SubAgent**:

Check `.kiro/steering/` status:
- **Bootstrap Mode**: Empty OR missing core files (product.md, tech.md, structure.md)
- **Sync Mode**: All core files exist

Use Glob to check for existing steering files.

## Invoke SubAgent

Delegate steering management to steering-agent:

Use the Task tool to invoke the SubAgent with file path patterns:

```
Task(
  subagent_type="steering-agent",
  description="Manage steering files",
  prompt="""
Mode: {bootstrap or sync based on detection}

File patterns to read:
- .kiro/steering/*.md (if sync mode)
- .kiro/settings/templates/steering/*.md
- .kiro/settings/rules/steering-principles.md

JIT Strategy: Fetch codebase files when needed, not upfront
"""
)
```

## Display Result

Show SubAgent summary to user:

### Bootstrap:
- Generated steering files: product.md, tech.md, structure.md
- Review and approve as Source of Truth

### Sync:
- Updated steering files
- Code drift warnings
- Recommendations for custom steering

## Notes

- All `.kiro/steering/*.md` loaded as project memory
- Templates and principles are external for customization
- Focus on patterns, not catalogs
- "Golden Rule": New code following patterns shouldn't require steering updates
