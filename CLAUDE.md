# CLAUDE.md

## Agent skills

Matt Pocock の engineering skills は `CLAUDE.md`及び`AGENTS.md` と `docs/agents/` に設定する。

- Issue tracker: GitHub Issues を使う。詳細は `docs/agents/issue-tracker.md` を見る。
- Triage labels: mattpocock/skills の default labels を使う。詳細は `docs/agents/triage-labels.md` を見る。
- Domain docs: single-context repo として root `CONTEXT.md` と `docs/adr/` を使う。詳細は `docs/agents/domain.md` を見る。

## Domain Context Preflight

Kiro / SDD / OpenSpec / 設計レビュー / 実装計画に入る前に、必ず以下を確認する。

- root `CONTEXT.md`
- root `CONTEXT-MAP.md` があれば関連 context
- `docs/adr/` のうち対象機能に関係する ADR
- 対象機能に近接する既存 `.kiro/specs/**`

`CONTEXT.md` の canonical terms / avoid terms / invariants と ADR の制約は、requirements / design / tasks / review / implementation の前提として扱う。
 
## Review exclusion settings

- 人間の明示的な許可なしに `.coderabbit.yml` / `.coderabbit.yaml` を変更しないこと。
- `.coderabbit.yml` / `.coderabbit.yaml` の `reviews.path_filters` に書かれた対象はレビューしたり変更しないこと。

Behavioral guidelines to reduce common LLM coding mistakes. Merge with project-specific instructions as needed.

**Tradeoff:** These guidelines bias toward caution over speed. For trivial tasks, use judgment.

## 1. Think Before Coding

**Don't assume. Don't hide confusion. Surface tradeoffs.**

Before implementing:
- State your assumptions explicitly. If uncertain, ask.
- If multiple interpretations exist, present them - don't pick silently.
- If a simpler approach exists, say so. Push back when warranted.
- If something is unclear, stop. Name what's confusing. Ask.

## 2. Simplicity First

**Minimum code that solves the problem. Nothing speculative.**

- No features beyond what was asked.
- No abstractions for single-use code.
- No "flexibility" or "configurability" that wasn't requested.
- No error handling for impossible scenarios.
- If you write 200 lines and it could be 50, rewrite it.

Ask yourself: "Would a senior engineer say this is overcomplicated?" If yes, simplify.

## 3. Surgical Changes

**Touch only what you must. Clean up only your own mess.**

When editing existing code:
- Don't "improve" adjacent code, comments, or formatting.
- Don't refactor things that aren't broken.
- Match existing style, even if you'd do it differently.
- If you notice unrelated dead code, mention it - don't delete it.

When your changes create orphans:
- Remove imports/variables/functions that YOUR changes made unused.
- Don't remove pre-existing dead code unless asked.

The test: Every changed line should trace directly to the user's request.

## 4. Goal-Driven Execution

**Define success criteria. Loop until verified.**

Transform tasks into verifiable goals:
- "Add validation" → "Write tests for invalid inputs, then make them pass"
- "Fix the bug" → "Write a test that reproduces it, then make it pass"
- "Refactor X" → "Ensure tests pass before and after"

For multi-step tasks, state a brief plan:
```
1. [Step] → verify: [check]
2. [Step] → verify: [check]
3. [Step] → verify: [check]
```

Strong success criteria let you loop independently. Weak criteria ("make it work") require constant clarification.

---

ref @CC-SDD-CLAUDE.md
