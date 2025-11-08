<!-- OPENSPEC:START -->
# OpenSpec Instructions

Instructions for AI coding assistants using OpenSpec for spec-driven development.

## TL;DR Quick Checklist

- Search existing work: `openspec spec list --long`, `openspec list` (use `rg` only for full-text search)
- Decide scope: new capability vs modify existing capability
- Pick a unique `change-id`: kebab-case, verb-led (`add-`, `update-`, `remove-`, `refactor-`)
- Scaffold: `proposal.md`, `tasks.md`, `design.md` (only if needed), and delta specs per affected capability
- Write deltas: use `## ADDED|MODIFIED|REMOVED Requirements`; include at least one `#### Scenario:` per requirement
- Validate: `openspec validate [change-id] --strict` and fix issues
- Request approval: Do not start implementation until proposal is approved

## Three-Stage Workflow

### Stage 1: Creating Changes
Create proposal when you need to:
- Add features or functionality
- Make breaking changes (API, schema)
- Change architecture or patterns  
- Optimize performance (changes behavior)
- Update security patterns

Triggers (examples):
- "Help me create a change proposal"
- "Help me plan a change"
- "Help me create a proposal"
- "I want to create a proposal"
- "I want to create a spec"

Loose matching guidance:
- Contains one of: `proposal`, `change`, `spec`
- With one of: `create`, `plan`, `make`, `start`, `help`

Skip proposal for:
- Bug fixes (restore intended behavior)
- Typos, formatting, comments
- Dependency updates (non-breaking)
- Configuration changes
- Tests for existing behavior

**Workflow**
1. Review `openspec/project.md`, `openspec list`, and `openspec list --specs` to understand current context.
2. Choose a unique verb-led `change-id` and scaffold `proposal.md`, `tasks.md`, optional `design.md`, and spec deltas under `openspec/changes/<id>/`.
3. Draft spec deltas using `## ADDED|MODIFIED|REMOVED Requirements` with at least one `#### Scenario:` per requirement.
4. Run `openspec validate <id> --strict` and resolve any issues before sharing the proposal.

### Stage 2: Implementing Changes
Track these steps as TODOs and complete them one by one.
1. **Read proposal.md** - Understand what's being built
2. **Read design.md** (if exists) - Review technical decisions
3. **Read tasks.md** - Get implementation checklist
4. **Implement tasks sequentially** - Complete in order
5. **Confirm completion** - Ensure every item in `tasks.md` is finished before updating statuses
6. **Update checklist** - After all work is done, set every task to `- [x]` so the list reflects reality
7. **Approval gate** - Do not start implementation until the proposal is reviewed and approved

### Stage 3: Archiving Changes
After deployment, create separate PR to:
- Move `changes/[name]/` → `changes/archive/YYYY-MM-DD-[name]/`
- Update `specs/` if capabilities changed
- Use `openspec archive <change-id> --skip-specs --yes` for tooling-only changes (always pass the change ID explicitly)
- Run `openspec validate --strict` to confirm the archived change passes checks

## Before Any Task

**Context Checklist:**
- [ ] Read relevant specs in `specs/[capability]/spec.md`
- [ ] Check pending changes in `changes/` for conflicts
- [ ] Read `openspec/project.md` for conventions
- [ ] Run `openspec list` to see active changes
- [ ] Run `openspec list --specs` to see existing capabilities

**Before Creating Specs:**
- Always check if capability already exists
- Prefer modifying existing specs over creating duplicates
- Use `openspec show [spec]` to review current state
- If request is ambiguous, ask 1–2 clarifying questions before scaffolding

### Search Guidance
- Enumerate specs: `openspec spec list --long` (or `--json` for scripts)
- Enumerate changes: `openspec list` (or `openspec change list --json` - deprecated but available)
- Show details:
  - Spec: `openspec show <spec-id> --type spec` (use `--json` for filters)
  - Change: `openspec show <change-id> --json --deltas-only`
- Full-text search (use ripgrep): `rg -n "Requirement:|Scenario:" openspec/specs`

## Quick Start

### CLI Commands

```bash
# Essential commands
openspec list                  # List active changes
openspec list --specs          # List specifications
openspec show [item]           # Display change or spec
openspec validate [item]       # Validate changes or specs
openspec archive <change-id> [--yes|-y]   # Archive after deployment (add --yes for non-interactive runs)

# Project management
openspec init [path]           # Initialize OpenSpec
openspec update [path]         # Update instruction files

# Interactive mode
openspec show                  # Prompts for selection
openspec validate              # Bulk validation mode

# Debugging
openspec show [change] --json --deltas-only
openspec validate [change] --strict
```

### Command Flags

- `--json` - Machine-readable output
- `--type change|spec` - Disambiguate items
- `--strict` - Comprehensive validation
- `--no-interactive` - Disable prompts
- `--skip-specs` - Archive without spec updates
- `--yes`/`-y` - Skip confirmation prompts (non-interactive archive)

## Directory Structure

```
openspec/
├── project.
[... omitted 198 of 454 lines ...]
```

Keep this managed block so 'openspec update' can refresh the instructions.

<!-- OPENSPEC:END -->

# Claude Code Spec-Driven Development

Kiro-style Spec Driven Development implementation using Claude Code slash commands, hooks, and agents.

## Project Context

### Paths
- Steering: `.kiro/steering/`
- Specs: `.kiro/specs/`
- Commands: `.claude/commands/`

### Steering vs Specification

**Steering** (`.kiro/steering/`) - Guide AI with project-wide rules and context
**Specs** (`.kiro/specs/`) - Formalize development process for individual features

### Active Specifications
- Check `.kiro/specs/` for active specifications
- Use `/kiro:spec-status [feature-name]` to check progress

## Development Guidelines
- Think in English, but generate responses in Japanese (プロジェクト規約と整合)

## Workflow

### Phase 0: Steering (Optional)
`/kiro:steering` - Create/update steering documents  
`/kiro:steering-custom` - Create custom steering for specialized contexts

Note: Optional for new features or small additions. You can proceed directly to spec-init.

### Phase 1: Specification Creation
1. `/kiro:spec-init [detailed description]` - Initialize spec with detailed project description
2. `/kiro:spec-requirements [feature]` - Generate requirements document
3. `/kiro:spec-design [feature]` - Interactive confirmation that requirements were reviewed
4. `/kiro:spec-tasks [feature]` - Interactive confirmation that both requirements and design were reviewed
5. `/kiro:validate-gap {feature}` (optional) - Validate changes against existing capabilities
6. `/kiro:validate-design {feature}` (optional) - Design review prior to tasks

### Phase 2: Implementation + Tracking
- `/kiro:spec-impl {feature} [tasks]` - Mark tasks as complete during implementation
- `/kiro:spec-status {feature}` - Check current progress and phase gates
- `/kiro:validate-impl {feature}` (optional) - Post-implementation validation

## Development Rules
1. Consider steering before major work; run `/kiro:steering` after significant architectural shifts.
2. Follow the 3-phase approval workflow: Requirements → Design → Tasks → Implementation.
3. Each phase requires human approval; use `-y` only when explicitly fast-tracking.
4. Do not skip phases: design requires approved requirements, tasks require approved design, implementation depends on approved tasks.
5. Update task status promptly; `tasks.md` should reflect real progress.
6. Keep steering synchronized with code and specs; add custom steering via `/kiro:steering-custom` for domain-specific policies.
7. Use `/kiro:spec-status` before starting work to ensure there are no conflicting active changes.

## Steering Configuration

### Default Steering Files
- `product.md`: Product context and business objectives
- `tech.md`: Technology stack and architectural decisions
- `structure.md`: File organization and code patterns

### Custom Steering Files
Managed through `/kiro:steering-custom`; entries specify inclusion mode (Always | Conditional | Manual) and path patterns.

### Inclusion Modes
- **Always**: Loaded in every interaction (default)
- **Conditional**: Loaded when file path matches provided glob patterns
- **Manual**: Explicitly referenced via `@filename.md`

---
- すべて日本語でやりとりすること
- ソースコード以外の生成されるファイルも日本語で記述すること

## 重要な注意事項

- **応対言語**: 必ず日本語で応対すること
- **コメント言語**: rustdoc(`///`や`//!`)は英語、それ以外のコメントやドキュメントは日本語で記述すること
- **タスクの完了条件**: テストはすべてパスすること
- **テストの扱い**: 行うべきテストをコメントアウトしたり無視したりしないこと
- **実装方針**:
  - 既存の多くの実装を参考にして、一貫性のあるコードを書くこと
  - protoactor-go(@references/protoactor-go), pekko(@references/pekko)の実装を参考にすること（Goの実装からRustイディオムに変換）
- ランタイム本体で `#[cfg(feature = "std")]` による機能分岐を入れないこと（テストコード内での使用は許容）
- **後方互換性**: 後方互換は不要（破壊的変更を恐れずに最適な設計を追求すること）
- **リリース状況**: まだ正式リリース前の開発フェーズ。必要であれば破壊的変更を歓迎し、最適な設計を優先すること。
- serena mcpを有効活用すること
- 当該ディレクトリ以外を読まないこと
- mod.rs禁止。2018モジュールを使え
- 単体テストは hoge.rs に対して hoge/tests.rs に記述すること
- 1ファイルに複数構造体、複数traitを記述しないこと
- 全タスクを完了した段階で `./scripts/ci-check.sh all` を実行し、エラーがないことを確認すること（途中工程では対象範囲のテストに留めてよい）

# AI-DLC and Spec-Driven Development

Kiro-style Spec Driven Development implementation on AI-DLC (AI Development Life Cycle)

## Project Context

### Paths
- Steering: `.kiro/steering/`
- Specs: `.kiro/specs/`

### Steering vs Specification

**Steering** (`.kiro/steering/`) - Guide AI with project-wide rules and context
**Specs** (`.kiro/specs/`) - Formalize development process for individual features

### Active Specifications
- Check `.kiro/specs/` for active specifications
- Use `/kiro:spec-status [feature-name]` to check progress

## Development Guidelines
- Think in English, but generate responses in Japanese (思考は英語、回答の生成は日本語で行うように)

## Minimal Workflow
- Phase 0 (optional): `/kiro:steering`, `/kiro:steering-custom`
- Phase 1 (Specification):
  - `/kiro:spec-init "description"`
  - `/kiro:spec-requirements {feature}`
  - `/kiro:validate-gap {feature}` (optional: for existing codebase)
  - `/kiro:spec-design {feature} [-y]`
  - `/kiro:validate-design {feature}` (optional: design review)
  - `/kiro:spec-tasks {feature} [-y]`
- Phase 2 (Implementation): `/kiro:spec-impl {feature} [tasks]`
  - `/kiro:validate-impl {feature}` (optional: after implementation)
- Progress check: `/kiro:spec-status {feature}` (use anytime)

## Development Rules
- 3-phase approval workflow: Requirements → Design → Tasks → Implementation
- Human review required each phase; use `-y` only for intentional fast-track
- Keep steering current and verify alignment with `/kiro:spec-status`

## Steering Configuration
- Load entire `.kiro/steering/` as project memory
- Default files: `product.md`, `tech.md`, `structure.md`
- Custom files are supported (managed via `/kiro:steering-custom`)
