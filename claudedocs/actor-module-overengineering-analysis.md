# modules/actor 過剰設計分析レポート

**作成日**: 2026-01-29
**分析対象**: `modules/actor/src`
**レビュアー**: Claude + Codex Architect Expert

---

## エグゼクティブサマリー

modules/actor の構造は、参照実装（pekko/protoactor-go）の規模感に比べて**細分化が進みすぎており**、特に `message_adapter` と `tick_driver` で「Less is more」「YAGNI」から逸脱している兆候が強い。no_std/std 二層や Rust の所有権要件を踏まえても、**公開型とファイル数が過剰で保守コストが高く、段階的な統合・非公開化が妥当**と判断される。

---

## 定量的指標

### 全体統計

| 指標 | 値 |
|------|-----|
| Rustファイル数 | 494 |
| ディレクトリ数 | 147 |
| 総行数 | 38,102行 |
| 平均行数/ファイル | 約77行 |

### ファイルサイズ分布（テスト除く 393ファイル）

| サイズ | ファイル数 | 割合 | 評価 |
|--------|-----------|------|------|
| 30行以下 | 138 | 35% | **要注意** |
| 31-100行 | 182 | 46% | 許容範囲 |
| 101行以上 | 73 | 19% | 適正 |

### 特定パターンのファイル数

| パターン | ファイル数 |
|----------|-----------|
| `*_shared.rs` | 20 |
| `*_handle*.rs` | 10 |
| `*_error*.rs` | 25 |
| `*_id.rs` | 7 |
| `tests.rs` | 101 |

---

## 参照実装との比較

### message_adapter

| 実装 | 型/モジュール数 |
|------|----------------|
| **pekko** | 2型（`AdaptWithRegisteredMessageAdapter`, `AdaptMessage`） |
| **fraktor-rs** | 13モジュール |

### 詳細な型の比較

**pekko（Scala）**:
```scala
// たった2つの内部型
case class AdaptWithRegisteredMessageAdapter[U](msg: U)
case class AdaptMessage[U, T](message: U, adapter: U => T)
```

**fraktor-rs（Rust）**:
- `AdapterError`: RegistryFull, TypeMismatch, EnvelopeCorrupted, ActorUnavailable, RegistryUnavailable
- `AdapterFailure`: TypeMismatch, Custom（**TypeMismatchが重複**）
- `AdapterFailureEvent`: 単純な `(Pid, AdapterFailure)` ラッパー
- `AdapterOutcome`: Converted | Failure | NotFound
- `AdapterEntry`
- `AdapterEnvelope`
- `AdapterPayload`
- `AdapterRefHandle`
- `AdapterRefHandleId`
- `AdapterRefSender`
- `registry`
- `adapt_message`

---

## 問題のあるモジュールの詳細分析

### 1. message_adapter（懸念度: 高）

**問題点**:
- 13サブモジュール + 薄い型が多数
- `AdapterError` と `AdapterFailure` で `TypeMismatch` が重複
- `AdapterFailureEvent` は単純なタプル構造体で、別ファイルの必要性が薄い
- pekko の2型相当に対して過剰な構造化

**具体例**:
```rust
// adapter_failure_event.rs - たった34行
pub struct AdapterFailureEvent {
  pid:     Pid,
  failure: AdapterFailure,
}
```

### 2. tick_driver（懸念度: 高）

**問題点**:
- 29サブモジュール
- 10〜20行級の公開enum/newtypeが多数
- 型数が増える割に振る舞い差が小さい
- **YAGNI逸脱の典型パターン**

**具体例**:
```rust
// tick_driver_id.rs - 20行
pub struct TickDriverId(u64);

// tick_driver_kind.rs - 19行
pub enum TickDriverKind {
  Auto,
  Hardware { source: HardwareKind },
  ManualTest,
}

// auto_profile_kind.rs - 13行
pub enum AutoProfileKind {
  Tokio,
  Embassy,
  Custom,
}

// tick_metrics_mode.rs - 22行
pub enum TickMetricsMode {
  AutoPublish { interval: Duration },
  OnDemand,
}
```

### 3. mailbox（懸念度: 中）

**問題点**:
- 19サブモジュール
- 並行制御やキュー管理の分割自体は妥当
- しかし薄いfuture/state型が散らばりすぎており結合・発見性が悪い

---

## 「1 file = 1 public type」ルールの影響

プロジェクトルールに従った結果ではあるが、以下の問題が生じている：

1. **ナビゲーションの困難**: 147ディレクトリを行き来するのは認知負荷が高い
2. **依存関係の把握が困難**: 細分化により `use` 文が増加
3. **小型型への過剰適用**: ≤30行ファイル比率35%は構造の過度な分割を示唆

---

## Architect Expert（Codex/GPT）の見解

### 結論

> 現状の modules/actor 構造は、参照実装（pekko/protoactor-go）の規模感に比べて細分化が進みすぎており、特に message_adapter と tick_driver で「Less is more」「YAGNI」から逸脱している兆候が強い。no_std/std 二層や Rust の所有権要件を踏まえても、公開型とファイル数が過剰で保守コストが高く、段階的な統合・非公開化が妥当。

### 懸念レベル別分析

| レベル | 対象 | 問題点 |
|--------|------|--------|
| **高** | message_adapter | 13サブモジュール + 薄い型多数。pekko の2型相当に対して過剰な構造化 |
| **高** | tick_driver | 10〜20行級の公開enum/newtypeが多数。ドメインプリミティブとして必要なものは維持し、不要な公開だけ整理する前提 |
| **中** | mailbox | 薄いfuture/state型が散らばりすぎ |
| **中** | ルール運用 | 「1 file = 1 public type」原則が小型型にまで厳密適用 |
| **低** | *_shared/*_handle | 必要性の薄い箇所まで横展開されていないか点検が必要 |

---

## 推奨アクションプラン

### 進捗チェック（完了/未完了）

- [x] Phase 1: 公開API監査（優先度: 高）
- [x] Phase 2: message_adapter 統合（優先度: 高）
- [ ] Phase 3: tick_driver ドメインプリミティブ保全（優先度: 高）
- [ ] Phase 4: mailbox 整理（優先度: 中）
- [ ] Phase 5: ルール運用見直し（優先度: 中）

※Phase 1 は 2026-01-30 に監査結果を追記済み。Phase 2 は 2026-01-30 に統合完了。Phase 3 は 2026-02-03 に方針を「ドメインプリミティブ保全」へ変更し、未完了。Phase 4 以降は未完了。

### Phase 1: 公開API監査（優先度: 高）

対象: `message_adapter` / `tick_driver` / `mailbox`

- `pub` 型を棚卸しし、外部利用有無で分類
  - `pub`: 外部APIとして必要
  - `pub(crate)`: 内部でのみ使用
  - 非公開: 特定モジュール内でのみ使用

#### 監査結果（2026-01-30）

判定基準: リポジトリ内の参照と公開シグネチャ。外部クレートでの利用有無は未調査。

##### message_adapter（公開型: 9）

- 外部API（`core::typed` から再エクスポート/公開シグネチャに登場）:
  `AdapterError`, `AdapterFailure`, `AdapterPayload`, `MessageAdapterRegistry`
- EventStream 経由で露出:
  `AdapterFailureEvent`（`EventStreamEvent::AdapterFailure`）
- 内部用途のみ（現状の参照は core 内部・テスト）:
  `AdaptMessage`, `AdapterEnvelope`, `AdapterLifecycleState`, `AdapterRefHandleId`

##### tick_driver（公開型: 27）

- 外部API/公開シグネチャに登場:
  `TickDriverConfig`, `TickDriverBundle`, `TickDriverError`, `TickDriverControl`, `TickDriverFactory`, `TickDriver`,
  `TickDriverProvisioningContext`, `TickDriverHandleGeneric`, `TickDriverId`, `TickDriverKind`, `TickDriverMetadata`,
  `AutoDriverMetadata`, `AutoProfileKind`, `SchedulerTickMetrics`, `TickExecutorSignal`, `TickFeed`
- 内部実装用途（core/std 実装でのみ参照）:
  `SchedulerTickExecutor`, `SchedulerTickMetricsProbe`, `SchedulerTickHandleOwned`, `TickDriverBootstrap`
- テスト/feature test-support 専用:
  `ManualTestDriver`, `ManualTickController`
- 再エクスポートのみ/参照なし:
  `HardwareKind`, `HardwareTickDriver`, `TickDriverGuideEntry`, `TickMetricsMode`, `TickPulseHandler`, `TickPulseSource`

##### mailbox（公開型: 21）

- 外部API/公開シグネチャに登場:
  `MailboxGeneric`, `MailboxesGeneric`, `MailboxPolicy`, `MailboxOverflowStrategy`, `MailboxRegistryError`,
  `MailboxMetricsEvent`, `MailboxPressureEvent`
- 内部実装用途（dispatcher/actor_cell などで参照）:
  `BackpressurePublisherGeneric`, `EnqueueOutcome`, `MailboxCapacity`, `MailboxInstrumentationGeneric`,
  `MailboxMessage`, `MailboxOfferFutureGeneric`, `MailboxPollFutureGeneric`, `ScheduleHints`
- 再エクスポートのみ/参照なし:
  `MailboxScheduleState`, `QueueOfferFuture`, `QueuePollFuture`, `QueueState`, `QueueStateHandle`, `SystemQueue`

### Phase 2: message_adapter 統合（優先度: 高）

1. `AdapterError` と `AdapterFailure` を単一 error/理由型へ統合
2. `AdapterFailureEvent` をイベントストリーム既存型に吸収
3. `AdapterRefHandleId` 等の newtype を private 化または type alias 化
4. 公開型数を削減

### Phase 3: tick_driver ドメインプリミティブ保全（優先度: 高）

1. `TickDriverId` / `TickDriverKind` / `HardwareKind` / `AutoProfileKind` / `TickMetricsMode` などのドメインプリミティブは統合しない
2. 公開APIの露出を整理する（`pub(crate)` 化、再エクスポートの整理、未使用の公開型の見直し）
3. ドキュメントとガイドは型境界を前提に更新し、境界でのみプリミティブへ変換する

### Phase 4: mailbox 整理（優先度: 中）

1. queue/state/future などの薄い型は所有型の近くにまとめる
2. アルゴリズム差分（overflow strategy 等）だけを独立モジュール化

### Phase 5: ルール運用見直し（優先度: 中）

「1 file = 1 public type」の運用について：
- 例外を設けるなら極小の設定enum/newtypeに限定
- 守るなら公開型そのものを削減する方針で整合を取る

---

## 工数見積もり

**Medium（1〜2日）**

---

## リスクと緩和策

| リスク | 緩和策 |
|--------|--------|
| 公開APIの破壊的変更が広範囲に波及 | モジュール単位で順次統合、利用側とテストを同時更新 |
| ドメインプリミティブの統合で型安全性が下がる | 統合は行わず、公開範囲の整理に限定する |
| no_std/core に std 依存が混入 | 統合先を core に寄せつつ std 依存型は std 側に限定維持 |

---

## 結論

modules/actor は**過剰設計の兆候が明確にある**。特に以下の点で改善が必要：

1. **message_adapter**: pekkoの2型に対して13モジュールは過剰。統合を推奨。
2. **tick_driver**: 29モジュール中、多くが10〜20行の小型型。ドメインプリミティブは維持し、公開範囲と再エクスポートを整理する。
3. **35%のファイルが30行以下**: 「1 file = 1 public type」ルールの過剰適用の可能性。

プロジェクトの設計価値観「Less is more」「YAGNI」に立ち返り、段階的な統合・非公開化を進めることを推奨する。
