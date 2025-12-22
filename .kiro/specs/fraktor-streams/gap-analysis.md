# ギャップ分析: fraktor-streams

## 前提
- 要件は生成済みだが未承認のため、分析結果は要件調整の材料として扱う。

## 1. 現状調査（既存アセット）

### 主要コンポーネント
- no_std/標準の境界と同期基盤: `modules/utils/src/core/runtime_toolbox.rs`, `modules/utils/src/core/sync/*`
- 共有キュー/バックプレッシャ基盤: `modules/utils/src/core/collections/queue/*`, `overflow_policy.rs`
- 非同期キュー: `modules/utils/src/core/collections/queue/async_queue_shared.rs`
- スケジューラ/ティック駆動: `modules/actor/src/core/scheduler/tick_driver/*`
- 完了通知プリミティブ: `modules/actor/src/core/futures/actor_future.rs`
- イベント配信: `modules/actor/src/core/event_stream/*`
- リモートのバックプレッシャ通知: `modules/actor/src/core/event_stream/backpressure_signal.rs`

### 観測されたパターン/制約
- `core` は no_std、`std` 実装は `std` 配下に隔離する運用が徹底されている。
- `ArcShared` と `ToolboxMutex` による共有ラッパ設計が既存の標準パターン。
- キュー/スタックは `modules/utils` の共通実装を使うことが必須。
- `mod.rs` 禁止/1ファイル1型/テストは `tests.rs` など lint 制約が強い。

### 既存の統合ポイント
- `RuntimeToolbox` が時間と同期原語を提供しており、no_std の実行基盤抽象として利用可能。
- `TickDriver` と `Scheduler` が周期駆動の実行基盤として利用可能。
- `EventStream` は状態・診断イベントの観測経路として再利用できる。
- リモート側にバックプレッシャ通知の概念（`BackpressureSignal`）が存在する。

## 2. 要件対応マップ（Requirement-to-Asset Map）

| 要件 | 既存アセット | 充足状況 | ギャップ種別 | ギャップ/備考 |
|---|---|---|---|---|
| 要件1: コアストリーム API | 型安全 API パターン（`modules/actor/src/core/typed/*`） | **Missing** | Missing | Source/Flow/Sink のドメイン抽象が存在しない。型安全の方針は流用可能。 |
| 要件2: グラフ合成とマテリアライズ値 | 共有ラッパ/ArcShared パターン | **Missing** | Missing | 合成規則・マテリアライズ値のモデルが未定義。 |
| 要件3: Materializer ライフサイクル | `TickDriver`/`Scheduler`、`ActorFuture` | **Partial** | Constraint | start/stop の設計パターンはあるが、Materializer の責務・状態モデルがない。 |
| 要件4: バックプレッシャと需要制御 | `modules/utils` のキュー/OverflowPolicy、`BackpressureSignal` | **Partial** | Constraint | キュー基盤はあるが、需要伝播・ストリームレベルのバックプレッシャは未実装。 |
| 要件5: 完了/キャンセル/エラー伝播 | `ActorFuture`, `QueueError`, `close` 操作 | **Partial** | Missing | 完了/失敗/キャンセルの伝播規約が未定義。 |
| 要件6: core/std 境界と no_std 互換 | core/std 分離の既存ルールと lint 群 | **Partial** | Constraint | 境界ルールは明確だが、新規ストリームモジュールの配置設計が必要。 |
| 要件7: std 拡張の実行統合 | `StdToolbox`, `tokio` 実装群 | **Partial** | Missing | std 側の Materializer/実行ブリッジが存在しない。 |

## 3. ギャップと制約

### 明確な不足（Missing）
- Source/Flow/Sink/Graph/Materializer のドメインモデル。
- マテリアライズ値の合成規則と API。
- ストリーム実行の開始/停止と状態管理の実装。
- std 側の実行ブリッジ（Tokio など）とサンプル/テスト。

### 仕様上の空白（Unknown/Decision Needed）
- 需要伝播（pull/push）モデルとバッファ方針の選定。
- Materializer が担う責務の境界（Scheduler/ActorSystem との連携度）。
- no_std 環境での実行モデル（ポーリング/手動ステップ/外部駆動）。
- マテリアライズ値の合成規則（右優先/左優先/ペア合成など）。

### 既存制約
- `core` に `#[cfg(feature = "std")]` を置けないため、std 実装は `std` へ完全分離が必須。
- `Queue/Stack` は `modules/utils` の実装を必ず再利用する必要がある。
- 共有は `ArcShared`/`ToolboxMutex` を標準とし、`&mut self` 設計が原則。

## 4. 実装アプローチの選択肢

### Option A: 既存クレート拡張（actor/core へストリーム機能を追加）
**概要**: `modules/actor/src/core` にストリーム抽象を追加し、std 側に Materializer 実装を追加する  
**利点**: 既存の Scheduler/EventStream と密結合で利用できる  
**欠点**: actor/core の責務肥大化、将来の分離が難しい

### Option B: 新規クレート `fraktor-streams-rs` を追加
**概要**: `modules/streams` を新設し、`core`/`std` を独立させる  
**利点**: 責務分離が明確で、no_std と std の境界設計が素直  
**欠点**: 新規 API/依存設計が必要、初期設計コストが高い

### Option C: ハイブリッド（最小コア + std ブリッジのみ先行）
**概要**: core に最小 API を定義し、std Materializer を先に提供して段階拡張  
**利点**: YAGNI に沿った段階実装が可能  
**欠点**: 後から API 変更が発生しやすい

## 5. 複雑度/リスク評価
- **Effort**: L（1–2週間）  
  - 新規ドメイン設計 + core/std 分離 + テスト/サンプルが必要
- **Risk**: High  
  - 実行モデルとバックプレッシャ設計が全体アーキテクチャに影響

## 6. Research Needed（設計フェーズ持ち越し）
- Pekko Streams の最小 API セットと Materializer の責務整理
- no_std での実行駆動モデル（外部ランナー/手動ポーリング/ティック駆動）
- バックプレッシャ実装の最小構成（キュー選定・需要伝播規則）
- マテリアライズ値合成の規則（型安全/合成順序）

## 7. 設計フェーズへの提案
- **推奨検討**: Option B または Option C を前提に core と std を厳密分離する方向で設計検討  
- **要決定事項**:
  - 需要伝播モデルとバッファ方針
  - Materializer のライフサイクルと統合先（Scheduler/ActorSystem 連携）
  - std 実装の最小統合範囲（Tokio 前提か、抽象層を設けるか）
