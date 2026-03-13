## ADDED Requirements

### Requirement: TAKT artifacts remain structurally valid
Repository-managed TAKT pieces, instructions, and output contracts SHALL remain structurally valid for the TAKT parser and SHALL not contain malformed indentation or fence nesting that changes schema meaning.

#### Scenario: Movement routing rules are sibling fields
- **WHEN** a movement defines `output_contracts` and `rules`
- **THEN** both fields SHALL appear at the correct sibling indentation and each `next` entry SHALL be nested under its corresponding rule item

#### Scenario: Output contract templates do not break code fences
- **WHEN** an output contract embeds example code blocks inside a fenced template
- **THEN** it SHALL use non-conflicting fence delimiters so the template remains parseable as one document

### Requirement: TAKT instruction inventory is wired consistently
Repository-managed TAKT instruction files SHALL either be referenced by an active piece or be removed from the tree.

#### Scenario: No orphan instruction files remain
- **WHEN** a TAKT instruction file exists under `.takt/facets/instructions`
- **THEN** at least one active piece SHALL reference it, or the file SHALL be removed as dead configuration

### Requirement: AI-mode cargo execution flows through guarded wrappers
CI helper scripts SHALL route every AI-mode `cargo` execution path through the shared guarded wrapper so timeout and hang-suspect protections apply consistently.

#### Scenario: Example execution is guarded in AI mode
- **WHEN** `./scripts/ci-check.sh ai all` or an equivalent AI-mode example path runs `cargo`
- **THEN** that execution SHALL go through the shared guarded wrapper rather than calling `cargo` directly
