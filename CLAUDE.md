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
