- すべて日本語でやりとりすること。ソースコード以外の生成されるファイルも日本語で記述すること
- 既存の多くの実装を参考にして、一貫性のあるコードを書くこと
- **後方互換性**: 後方互換は不要（破壊的変更を恐れずに最適な設計を追求すること）
- **リリース状況**: まだ正式リリース前の開発フェーズ。必要であれば破壊的変更を歓迎し、最適な設計を優先すること
- serena mcpを有効活用すること
- 当該ディレクトリ以外を読まないこと
- 一連のタスクが完了した最後でかつソースコードを編集した場合は `./scripts/ci-check.sh ai all` を実行し、エラーがないことを確認すること（途中工程では対象範囲のテストに留めてよい）。ただし **TAKT ピース実行中は、`final-ci` ムーブメント以外で `./scripts/ci-check.sh ai all` を実行してはならない**（各ムーブメントのインストラクションに従うこと）。
- `./scripts/ci-check.sh ai all` は所要時間が長いが、完了を待ってください。
- `./scripts/ci-check.sh`は内部で`cargo`を呼び出すので並行実行できません。
- CHANGELOG.mdはgithub actionが自動的に作るのでAIエージェントは編集してはならない
- **中間アーティファクト配置**:
  - **takt 実行時の中間アーティファクト** (takt が生成するレポート・分析・決定ログ等) は `.takt/` 配下に配置すること。プロジェクトルート直下やソースツリー内（`reports/` 等）に作成してはならない。
  - **計画ドキュメント** (人間/AI が書く investigation / plan / design notes) は `docs/plan/` に配置すること。`.takt/` には置かない (takt 中間生成物専用のため)。
- lintエラーを安易にallowなどで回避しないこと。allowを付ける場合は人間から許可を得ること
- TOCTOUを避ける設計をすること
- 優先順位や依存関係を考慮した上でボーイスカウトルールを適用すること

# 基本原則

- シンプルさの優先: すべての変更を可能な限りシンプルに保ち、コードへの影響範囲を最小限に抑える。
- 妥協の排除: 根本原因を特定すること。一時しのぎの修正は行わず、シニア開発者としての基準を維持する。
- 影響の最小化: 必要な箇所のみを変更し、新たなバグの混入を徹底的に防ぐ。

## 設計・命名・構造ルール（.claude/rules/rust/）

詳細は `.claude/rules/rust/` に集約されている。変更する場合は人間から許可を取ること：

| ファイル | 内容 |
|----------|------|
| `immutability-policy.md` | 内部可変性禁止、&mut self 原則、AShared パターン |
| `cqs-principle.md` | CQS 原則、違反判定フロー |
| `type-organization.md` | 1file1type + 例外基準、公開範囲の判断フロー |
| `naming-conventions.md` | 曖昧サフィックス禁止、Shared/Handle 命名、ドキュメント言語 |
| `reference-implementation.md` | protoactor-go/pekko 参照手順、Go/Scala → Rust 変換 |
| `module-structure.md` | modules/*/src/core（no_std）と std（アダプタ）の分離構造 |

## Dylint lint（8つ、機械的強制）

mod-file, module-wiring, type-per-file, tests-location, use-placement, rustdoc, cfg-std-forbid, ambiguous-suffix

## AI-DLC and Spec-Driven Development
@.agents/CC-SDD.md を読むこと

<!-- rtk-instructions v2 -->
# RTK (Rust Token Killer) - Token-Optimized Commands

## Golden Rule

**Always prefix commands with `rtk`**. If RTK has a dedicated filter, it uses it. If not, it passes through unchanged. This means RTK is always safe to use.

**Important**: Even in command chains with `&&`, use `rtk`:
```bash
# ❌ Wrong
git add . && git commit -m "msg" && git push

# ✅ Correct
rtk git add . && rtk git commit -m "msg" && rtk git push
```

## RTK Commands by Workflow

### Build & Compile (80-90% savings)
```bash
rtk cargo build         # Cargo build output
rtk cargo check         # Cargo check output
rtk cargo clippy        # Clippy warnings grouped by file (80%)
rtk tsc                 # TypeScript errors grouped by file/code (83%)
rtk lint                # ESLint/Biome violations grouped (84%)
rtk prettier --check    # Files needing format only (70%)
rtk next build          # Next.js build with route metrics (87%)
```

### Test (90-99% savings)
```bash
rtk cargo test          # Cargo test failures only (90%)
rtk vitest run          # Vitest failures only (99.5%)
rtk playwright test     # Playwright failures only (94%)
rtk test <cmd>          # Generic test wrapper - failures only
```

### Git (59-80% savings)
```bash
rtk git status          # Compact status
rtk git log             # Compact log (works with all git flags)
rtk git diff            # Compact diff (80%)
rtk git show            # Compact show (80%)
rtk git add             # Ultra-compact confirmations (59%)
rtk git commit          # Ultra-compact confirmations (59%)
rtk git push            # Ultra-compact confirmations
rtk git pull            # Ultra-compact confirmations
rtk git branch          # Compact branch list
rtk git fetch           # Compact fetch
rtk git stash           # Compact stash
rtk git worktree        # Compact worktree
```

Note: Git passthrough works for ALL subcommands, even those not explicitly listed.

### GitHub (26-87% savings)
```bash
rtk gh pr view <num>    # Compact PR view (87%)
rtk gh pr checks        # Compact PR checks (79%)
rtk gh run list         # Compact workflow runs (82%)
rtk gh issue list       # Compact issue list (80%)
rtk gh api              # Compact API responses (26%)
```

### JavaScript/TypeScript Tooling (70-90% savings)
```bash
rtk pnpm list           # Compact dependency tree (70%)
rtk pnpm outdated       # Compact outdated packages (80%)
rtk pnpm install        # Compact install output (90%)
rtk npm run <script>    # Compact npm script output
rtk npx <cmd>           # Compact npx command output
rtk prisma              # Prisma without ASCII art (88%)
```

### Files & Search (60-75% savings)
```bash
rtk ls <path>           # Tree format, compact (65%)
rtk read <file>         # Code reading with filtering (60%)
rtk grep <pattern>      # Search grouped by file (75%)
rtk find <pattern>      # Find grouped by directory (70%)
```

### Analysis & Debug (70-90% savings)
```bash
rtk err <cmd>           # Filter errors only from any command
rtk log <file>          # Deduplicated logs with counts
rtk json <file>         # JSON structure without values
rtk deps                # Dependency overview
rtk env                 # Environment variables compact
rtk summary <cmd>       # Smart summary of command output
rtk diff                # Ultra-compact diffs
```

### Infrastructure (85% savings)
```bash
rtk docker ps           # Compact container list
rtk docker images       # Compact image list
rtk docker logs <c>     # Deduplicated logs
rtk kubectl get         # Compact resource list
rtk kubectl logs        # Deduplicated pod logs
```

### Network (65-70% savings)
```bash
rtk curl <url>          # Compact HTTP responses (70%)
rtk wget <url>          # Compact download output (65%)
```

### Meta Commands
```bash
rtk gain                # View token savings statistics
rtk gain --history      # View command history with savings
rtk discover            # Analyze Claude Code sessions for missed RTK usage
rtk proxy <cmd>         # Run command without filtering (for debugging)
rtk init                # Add RTK instructions to CLAUDE.md
rtk init --global       # Add RTK to ~/.claude/CLAUDE.md
```

## Token Savings Overview

| Category | Commands | Typical Savings |
|----------|----------|-----------------|
| Tests | vitest, playwright, cargo test | 90-99% |
| Build | next, tsc, lint, prettier | 70-87% |
| Git | status, log, diff, add, commit | 59-80% |
| GitHub | gh pr, gh run, gh issue | 26-87% |
| Package Managers | pnpm, npm, npx | 70-90% |
| Files | ls, read, grep, find | 60-75% |
| Infrastructure | docker, kubectl | 85% |
| Network | curl, wget | 65-70% |

Overall average: **60-90% token reduction** on common development operations.
<!-- /rtk-instructions -->