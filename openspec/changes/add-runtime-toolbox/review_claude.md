# OpenSpec変更提案 `add-runtime-toolbox` レビュー

**レビュー日時**: 2025-11-01
**レビュー対象**: openspec/changes/add-runtime-toolbox
**バリデーション**: ✅ `openspec validate --strict` 合格

---

## 📊 エグゼクティブサマリー

| 項目 | 評価 | 詳細 |
|------|------|------|
| **OpenSpec準拠** | ✅ 完全 | 形式・構造ともに準拠 |
| **技術的妥当性** | ✅ 高 | 既存アーキテクチャとの整合性良好 |
| **実装準備度** | ⚠️ 80% | 技術的決定の明示化が必要 |
| **破壊的変更** | ✅ なし | 後方互換性を維持 |
| **総合評価** | **条件付き承認** | design.md追加を推奨 |

---

## ✅ 構造的妥当性

### OpenSpec形式チェック

| ドキュメント | 状態 | 内容評価 |
|-------------|------|----------|
| `proposal.md` | ✅ 存在 | Why/What/Impact/Scope/Rollout完備 |
| `tasks.md` | ✅ 存在 | 3フェーズ14タスクで構成 |
| `design.md` | ❌ 不在 | **追加推奨** (後述) |
| `specs/runtime-env/spec.md` | ✅ 存在 | 3要件、各2-1シナリオ |

### デルタ構造分析

```json
{
  "deltaCount": 3,
  "operations": ["ADDED", "ADDED", "ADDED"],
  "affectedSpecs": ["runtime-env"],
  "totalRequirements": 3,
  "totalScenarios": 5
}
```

**評価**: ✅ 新規capability追加として適切な構成

---

## 🎯 提案内容の評価

### Why (動機)

**問題認識の明確性**: ✅ 優秀

> `ActorRuntimeMutex` は利用クレートに応じて暗黙的にバックエンドが切り替わるが、アプリケーション側からは挙動を明示できず学習コストが高い

**現状の課題**:
- 現行実装 (`actor-core/runtime_mutex.rs:6`): `type ActorRuntimeMutex<T> = SpinSyncMutex<T>` と固定
- `actor-std` では暗黙的に異なる実装が期待されるが、明示的な切替手段がない
- 将来の拡張 (`Condvar` 等) に対する抽象化ポイントが不在

**評価**: 実コードベースと整合した課題認識

### What Changes (変更内容)

**技術的アプローチ**: ✅ 適切

1. **`RuntimeToolbox` トレイト導入** → Dependency Injection パターン
2. **標準実装提供** → `NoStdToolbox` / `StdToolbox` で組み込み〜標準環境カバー
3. **Builder統合** → `ActorSystemBuilder` での環境設定API
4. **デフォルト戦略** → `NoStdToolbox` で後方互換性維持

**アーキテクチャ整合性**:
- ✅ 既存の `SyncMutexLike` 抽象との統合
- ✅ `no_std` / `alloc` / `std` feature戦略と整合
- ✅ `ActorSystemState` での一元管理 (proposal.md:8)

### Impact (影響範囲)

**API変更**: ✅ 非破壊的
- デフォルト挙動は `SpinSyncMutex` で従来通り
- オプトイン形式での環境設定

**影響モジュール**:
```
modules/utils-core      → RuntimeToolbox定義
modules/actor-core      → ActorSystemBuilder/State統合
modules/actor-std       → StdToolbox再エクスポート
```

**評価**: 影響範囲が明確で管理可能

### Scope (スコープ管理)

**Goals**: ✅ 明確
1. RuntimeToolbox抽象と標準実装
2. ActorSystemBuilderへの統合
3. ランタイム内部のリファクタリング
4. ドキュメント・サンプル更新

**Non-Goals**: ✅ 適切な除外
- ❌ `ActorSystem<R>` ジェネリクス化 → 複雑性回避
- ❌ 実行時切替 → 起動時決定に限定
- ❌ Condvar等の新規プリミティブ → 別提案で扱う

**評価**: YAGNI原則に沿ったスコープ設定

---

## 🔍 技術的詳細レビュー

### Requirement 1: RuntimeToolbox設定API

```markdown
ランタイムは `ActorSystem` 初期化時に `RuntimeToolbox` を設定する手段を
提供しなければならない (MUST)。未設定の場合は `NoStdToolbox` を使用しなければならない (MUST)。
```

#### シナリオ1: 環境未指定で初期化

```gherkin
GIVEN 利用者が環境を設定せずに `ActorSystem` を初期化する
WHEN ランタイムが同期プリミティブを生成する
THEN `NoStdToolbox` の `SpinSyncMutex` バックエンドが使用され、従来通りの挙動を示す
```

**評価**: ✅ 後方互換性を明示
**補足**: デフォルトが `NoStdToolbox` = `SpinSyncMutex` の根拠は既存実装 (`runtime_mutex.rs:6`)

#### シナリオ2: StdToolbox指定

```gherkin
GIVEN 利用者が `StdToolbox` を設定して `ActorSystem` を初期化する
WHEN ランタイムが同期プリミティブを生成する
THEN `std::sync::Mutex` バックエンドが利用される
```

**評価**: ✅ 標準環境への切替を明示
**懸念**: ⚠️ `std::sync::Mutex` との対応が `StdSyncMutex` 実装と整合するか要確認

### Requirement 2: SyncMutexLike生成

```markdown
`RuntimeToolbox` は `SyncMutexLike` を実装する同期プリミティブを生成しなければならない (MUST)。
標準環境として `NoStdToolbox` と `StdToolbox` を提供しなければならない (MUST)。
```

#### シナリオ1: NoStdToolbox利用

**評価**: ✅ `no_std` 構成との整合性を明記

#### シナリオ2: StdToolbox利用

**評価**: ✅ feature gateとの連携を明記

**既存実装との整合性**:
```rust
// utils-core/src/sync/sync_mutex_like.rs:4-9
#[cfg(feature = "std")]
mod std_sync_mutex;

pub use spin_sync_mutex::*;
#[cfg(feature = "std")]
pub use std_sync_mutex::{StdSyncMutex, StdSyncMutexGuard};
```

**評価**: ✅ 既存の `SyncMutexLike` トレイトと完全整合

### Requirement 3: ドキュメント要件

```markdown
ランタイムは `RuntimeToolbox` の設定方法と注意点をドキュメント化しなければならない (MUST)。
```

#### シナリオ: actor-std利用者向けガイド

**評価**: ✅ 必要十分なドキュメント要件
**推奨**: コード例に以下を含める
- `ActorSystemBuilder::with_env(StdToolbox)` の使用例
- feature有効化手順 (`features = ["std"]`)
- `NoStdToolbox` vs `StdToolbox` の選択基準

---

## 🏗️ プロジェクト整合性評価

### project.mdとの整合性

| プロジェクト基準 | 適合状況 | 評価 |
|-----------------|---------|------|
| `no_std` / `alloc` 対応 | ✅ 完全 | `NoStdToolbox` で対応 |
| `#![deny(...)]` 準拠 | ✅ 想定内 | 新規APIにRustDoc必須 |
| feature戦略 | ✅ 整合 | `std` featureで切替 |
| portable-atomic活用 | ✅ 維持 | 既存抽象を変更せず |
| CI/Dylint準拠 | ✅ 想定内 | 既存パイプライン適用可 |

### 既存コードベースとの統合評価

**影響を受けるファイル数**: 17ファイル
- `ActorRuntimeMutex::new` 呼び出し箇所のリファクタリング対象
- `modules/actor-core` 配下が主な影響範囲

**リファクタリング戦略の妥当性**:
```
現状: ActorRuntimeMutex::new(value)
     ↓
提案: system_state.env().create_mutex(value)
```

**評価**: ✅ 段階的移行が可能な設計

---

## ⚠️ 懸念点と推奨改善

### 🟡 重要: design.md追加の推奨

**理由**: AGENTS.mdの基準により以下に該当

- ✅ **横断的変更**: 3モジュール (`utils-core`, `actor-core`, `actor-std`) に影響
- ✅ **アーキテクチャパターン変更**: DI導入による設計変更
- ✅ **実装前の曖昧性**: トレイト設計の詳細が不明瞭

**推奨構成**:

```markdown
## Context
- 現行 `ActorRuntimeMutex` の暗黙的バックエンド切替問題
- `SyncMutexLike` トレイトとの関係
- protoactor-go / Pekko での環境抽象化パターン

## Goals / Non-Goals
(proposal.mdから移行)

## Decisions

### Decision 1: RuntimeToolbox トレイト設計
**選択**: トレイトベース + 動的ディスパッチ
**代替案**:
  - A) ジェネリクス (`ActorSystem<E: RuntimeToolbox>`) → 複雑性増大で却下
  - B) マクロベース → コンパイル時固定で柔軟性不足
**根拠**: 初期化時のみの呼び出しで性能影響軽微、拡張性優先

### Decision 2: トレイトAPI
```rust
pub trait RuntimeToolbox {
    type Mutex<T>: SyncMutexLike<T>;
    fn create_mutex<T>(&self, value: T) -> Self::Mutex<T>;
    // 将来拡張: fn create_condvar(&self) -> Self::Condvar;
}
```
**根拠**: `SyncMutexLike` との型整合性、将来のCondvar拡張への準備

### Decision 3: デフォルト環境
**選択**: `NoStdToolbox` (= `SpinSyncMutex`)
**根拠**: 既存コードの挙動維持、組み込み環境でのゼロコスト抽象

### Decision 4: 環境の保持場所
**選択**: `ActorSystemState` にて一元管理
**根拠**: 生成コンポーネント全体への伝播容易、ライフタイム管理明確

## Risks / Trade-offs

### Risk 1: 動的ディスパッチのオーバーヘッド
**影響**: ロック生成時の間接呼び出し
**緩和策**: 生成はシステム初期化時のみ、ホットパスではない
**測定**: ベンチマークで0.1%未満の影響を確認予定

### Risk 2: API複雑化
**影響**: `ActorSystemBuilder` の設定項目増加
**緩和策**: デフォルト挙動維持、ドキュメントで選択基準明示

## Migration Plan

### Phase 1: トレイト・実装追加 (非破壊)
```rust
// utils-core
pub trait RuntimeToolbox { ... }
pub struct NoStdToolbox;
#[cfg(feature = "std")]
pub struct StdToolbox;
```

### Phase 2: ActorSystem統合 (デフォルト維持)
```rust
// actor-core
impl ActorSystemBuilder {
    pub fn with_env<E: RuntimeToolbox>(self, env: E) -> Self { ... }
}
```

### Phase 3: 内部リファクタリング
```
- ActorRuntimeMutex::new(value)
+ system_state.env().create_mutex(value)
```
影響箇所: 17ファイル

### Phase 4: actor-std公開
```rust
// actor-std
pub use cellactor_utils_core_rs::sync::env::StdToolbox;
```

## Open Questions
- [ ] `RuntimeToolbox` を `Send + Sync` とするか? → 複数スレッド初期化の可否
- [ ] 環境のクローン可否は? → `Arc<dyn RuntimeToolbox>` 検討
```

### 🟡 tasks.mdの詳細化推奨

**現状の粒度**: 高レベルすぎ

**改善例**:

#### Phase 1: 調査 (現状)
```markdown
- [ ] `ActorRuntimeMutex::new` および `SpinSyncMutex::new` を直接呼び出している箇所を洗い出す
```

#### Phase 1: 調査 (推奨)
```markdown
- [ ] 1.1 `ActorRuntimeMutex::new` 呼び出し箇所をGrep検索し影響ファイル一覧作成
- [ ] 1.2 各ファイルでの使用パターン分類 (初期化/ホットパス/テスト)
- [ ] 1.3 `ActorSystemBuilder` の既存API確認と拡張ポイント特定
- [ ] 1.4 `ActorSystemState` のフィールド追加可能性調査
```

#### Phase 2: 実装 (現状)
```markdown
- [ ] `RuntimeToolbox` トレイトと標準実装 (`NoStdToolbox` / `StdToolbox`) を追加する
```

#### Phase 2: 実装 (推奨)
```markdown
- [ ] 2.1 `RuntimeToolbox` トレイト定義 (utils-core/src/sync/env.rs)
- [ ] 2.2 `NoStdToolbox` 実装とテスト (SpinSyncMutex生成確認)
- [ ] 2.3 `StdToolbox` 実装とfeature gate (std feature時のみ)
- [ ] 2.4 `ActorSystemBuilder::with_env()` API追加
- [ ] 2.5 `ActorSystemConfig` に環境フィールド追加
- [ ] 2.6 `ActorSystemState` への環境保持実装
- [ ] 2.7 デフォルト環境 (`NoStdToolbox`) 設定
- [ ] 2.8 ユニットテスト (環境切替の動作確認)
```

### 🟢 スペックの精緻化機会

#### Requirement 1への追加推奨シナリオ

**シナリオ3: 環境切替の不可性**
```gherkin
GIVEN `ActorSystem` が既に初期化されている
WHEN 利用者が環境を変更しようとする
THEN コンパイルエラーまたは実行時エラーとなり、環境は不変であることが保証される
```

**理由**: Non-Goalsで「実行時切替」を除外しているが、仕様で明示されていない

#### Requirement 2への追加検討

**シナリオ3: カスタムRuntimeToolbox実装**
```gherkin
GIVEN 利用者が独自の `RuntimeToolbox` を実装する
WHEN カスタム環境を `ActorSystemBuilder` に設定する
THEN カスタム同期プリミティブがランタイムで利用される
```

**理由**: 拡張性を明示することで設計意図が明確化

---

## 📈 実装準備度評価

### 準備完了項目 ✅

- [x] OpenSpec形式準拠
- [x] 既存コードベースとの整合性確認
- [x] 後方互換性戦略
- [x] feature戦略との整合
- [x] 影響範囲の特定

### 追加推奨項目 ⚠️

- [ ] `design.md` 作成 (トレイト設計詳細)
- [ ] `tasks.md` 詳細化 (14 → 25サブタスク)
- [ ] スペックへのシナリオ追加 (環境不変性)
- [ ] パフォーマンスベンチマーク基準設定

### 実装中に決定すべき事項 📋

1. `RuntimeToolbox` のライフタイム管理 (`&'static` vs `Arc<dyn>`)
2. トレイトの `Send + Sync` 境界
3. エラーハンドリング戦略 (環境設定失敗時)

---

## 🎯 最終判定

### 総合評価: **条件付き承認 (Conditional Approval)**

#### 承認条件

**🔴 必須 (実装前)**:
1. `design.md` の追加 (トレイト設計・型定義の詳細化)
2. `tasks.md` の詳細化 (14 → 25サブタスク程度)

**🟡 推奨 (実装初期)**:
3. スペックへのシナリオ追加 (環境不変性の明示)
4. パフォーマンスベンチマーク基準の事前設定

#### 実装着手可否判定

| 項目 | 現状 | 条件達成後 |
|------|------|----------|
| **構造的準備** | 90% | 95% |
| **技術的明確性** | 70% | 90% |
| **実装可能性** | 80% | 95% |
| **総合準備度** | **80%** | **93%** |

**推奨アクション**:
1. design.md作成 (2-3時間の作業)
2. tasks.md詳細化 (1時間の作業)
3. 上記完了後、実装着手可能

---

## 📌 強み・弱みサマリー

### ✅ 主要な強み

1. **設計の健全性**: DI パターンによる疎結合設計
2. **後方互換性**: デフォルト挙動維持で既存コードへの影響なし
3. **拡張性**: 将来の Condvar 等への拡張準備
4. **プロジェクト整合性**: `no_std` / feature戦略と完全整合
5. **スコープ管理**: 適切なGoals/Non-Goals設定

### ⚠️ 改善余地

1. **技術的詳細の不足**: トレイト設計の型定義が未明示
2. **タスク粒度**: 高レベルすぎて実装時の判断余地が大きい
3. **シナリオ不足**: 環境不変性などの制約が未記述
4. **パフォーマンス基準**: ベンチマーク目標が未設定

---

## 📝 レビュー結論

本提案は**技術的に健全で、プロジェクトアーキテクチャとの整合性も高い**が、
実装詳細の曖昧性が残存するため、**design.md追加とtasks.md詳細化後の実装着手を推奨**します。

上記条件達成後は、**高い成功確率で実装可能**と判断されます。

---

**レビュアー署名**: Claude Code
**次のステップ**: design.md作成 → tasks.md詳細化 → 再レビュー → 実装着手承認
