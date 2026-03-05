## GitHub Issue #413: refactor: actor scheduler/tick_driver の小型enum統合

## 背景

`modules/actor/src/core/scheduler/` が64公開型を持ち、actor モジュール全体（383型）の17%を占める。特に `tick_driver/` 配下に26個の公開型ファイルが集中している。

## 問題

### 小型 enum/struct の過剰分割

1file1type ルールにより、10〜18行程度の小型 enum が個別ファイルに分離されている:

| ファイル | 行数 | 内容 |
|----------|------|------|
| `batch_mode.rs` | 10 | `pub enum BatchMode { Immediate, Deferred }` |
| `runner_mode.rs` | 10 | `pub enum RunnerMode { Blocking, Yielding }` |
| `tick_driver_kind.rs` | ~15 | `pub enum TickDriverKind { ... }` |
| `hardware_kind.rs` | ~12 | `pub enum HardwareKind { ... }` |
| `auto_profile_kind.rs` | ~12 | `pub enum AutoProfileKind { ... }` |
| `task_run_summary.rs` | 8 | `pub struct TaskRunSummary { ... }` |

これらは `TickDriverConfig` のフィールド型としてのみ使われ、外部から直接参照されない。

### actorモジュール全体の20行以下ファイル

actorモジュール全体で **55個** のファイルが20行以下。構造的オーバーヘッドが大きい。

## タスク

- [ ] `tick_driver/` 配下の小型enumで、親型のフィールドとしてのみ使われるものを特定
- [ ] 型配置ルール（type-organization.md）の同居条件を満たすものを親型ファイルに統合
- [ ] scheduler サブモジュール全体で同様のパターンを適用
- [ ] 統合後のファイル数と型数を計測

## 期待効果

- scheduler 公開型数: 64 → ~40（推定）
- ファイル数: ~15ファイル削減
- ナビゲーション性の向上（細かすぎるファイル分割の解消）

## 優先度

**中〜低**（影響範囲が限定的。他のリファクタリングと並行可能）

### Labels
refactoring