# ギャップ分析: fraktor-streams

## 前提
- 要件/設計/タスクは未承認のため、分析結果は要件・設計の調整材料として扱う。
- `modules/streams` は骨組みのみで、実装は削除済み（または削除前提）とみなす。

## 1. 現状調査（既存アセット）

### 主要コンポーネント
- `modules/streams`:
  - `src/lib.rs` に core/std のモジュール定義のみ。`src/core.rs`/`src/std.rs` は空。
  - `Cargo.toml` に tokio 依存と例題エントリが残存するが、`examples/` 配下の実体は存在しない。
- `modules/utils`:
  - 共有キュー/OverflowPolicy: `modules/utils/src/core/collections/queue/*`
  - 共有同期/所有権: `modules/utils/src/core/sync/*`（`ArcShared`/`ToolboxMutex`）
- `modules/actor`:
  - ActorSystem std ラッパ: `modules/actor/src/std/system/base.rs`
  - TickDriver/スケジューラ: `modules/actor/src/core/scheduler/tick_driver/*`
  - 拡張登録機構: `modules/actor/src/core/extension/*`, `modules/actor/src/core/system/extensions*.rs`
  - EventStream: `modules/actor/src/core/event_stream/*`
- `modules/remote` / `modules/cluster`:
  - ActorSystem 側の remote/cluster が既存だが、streams は ActorSystem を通じて間接的に利用する前提。

### 観測されたパターン/制約
- `core` は no_std、`std` 実装は `std` 配下に隔離（`cfg-std-forbid`）。
- `mod.rs` 禁止、1ファイル1型、テストは `tests.rs` 配置。
- 共有は `ArcShared`/`ToolboxMutex` が標準。
- rustdoc は英語、それ以外は日本語。
- FQCN での import を徹底。

### 既存の統合ポイント
- ActorSystem の拡張登録（materializer を extension として登録可能）。
- TickDriver/スケジューラ（周期駆動の基盤として利用可能）。
- EventStream（診断/状態の観測経路）。

## 2. 要件対応マップ（Requirement-to-Asset Map）

| 要件 | 既存アセット | 充足状況 | ギャップ種別 | 備考 |
|---|---|---|---|---|
| 1.1-1.3 コアDSL/接続 | なし | **Missing** | Missing | Source/Flow/Sink/Graph が未実装。 |
| 1.4 DSL コンビネータ | なし | **Missing** | Missing | `map`/`flatMapConcat`/`single`/Sink群が未実装。 |
| 1.5 GraphStage 中核抽象 | なし | **Missing** | Missing | GraphStage/StageLogic の中核抽象が未実装。 |
| 2.1-2.3 合成/マテリアライズ | なし | **Missing** | Missing | RunnableGraph/MatCombine 等のモデルが未定義。 |
| 3.1-3.4 Materializer | ActorSystem 拡張機構 | **Partial** | Constraint | extension 機構はあるが streams 側の設計が未実装。 |
| 4.1-4.3 需要/バックプレッシャ | utils queue/OverflowPolicy | **Partial** | Constraint | 基盤はあるが需要伝播/契約が未実装。 |
| 5.1-5.3 完了/キャンセル/失敗 | EventStream/ActorFuture | **Missing** | Missing | ストリーム用の状態遷移が未定義。 |
| 6.1-6.3 core/std 境界 | lint/構造規約 | **Partial** | Constraint | 境界ルールは明確だが streams 実装は空。 |
| 6.4 actor core 再利用 | actor/core の Scheduler/TickDriver/Extension | **Missing** | Missing | streams/core が actor/core に依存する前提。 |
| 6.5 actor 依存の最小化 | - | **Missing** | Missing | 必要最小限の依存に留める設計規約が必要。 |
| 6.6 actor/core 依存方向禁止 | - | **Missing** | Constraint | actor/core → streams/core の依存を禁止。 |
| 6.7 actor 型の非露出 | - | **Missing** | Constraint | streams 公開 API の境界定義が必要。 |
| 7.1-7.5 Actor 実行統合 | ActorSystem/スケジューラ | **Partial** | Missing | ActorMaterializer/DriveActor が未実装。 |
| 8.1-8.5 examples | なし | **Missing** | Missing | examples ディレクトリが空。DSL+ActorSystem 例が必要。 |

## 3. ギャップと制約

### 明確な不足（Missing）
- Source/Flow/Sink と Graph/Shape のドメインモデル。
- DSL コンビネータ（`map`, `flatMapConcat`, `single`, Sink の `ignore`/`fold`/`head`/`last`/`foreach`）。
- GraphStage/StageLogic の中核抽象。
- Materializer / ActorMaterializer / StreamDriveActor の実装。
- 需要伝播・バックプレッシャ制御・完了/失敗/キャンセルの状態遷移。
- examples とテスト（DSL + ActorSystem）。

### 仕様上の空白（Unknown/Decision Needed）
- `flatMapConcat` の最小語義とステージ設計（Pekko のオペレータ分類に準拠）。
- `Materializer` が ActorSystem 拡張として登録される方式。
- ActorSystem の TickDriver/スケジューラを stream drive にどう結びつけるか。
- remote/cluster 有効時のスモーク構成（起動のみか、簡易駆動まで含めるか）。

### 既存制約
- `core` では `#[cfg(feature = "std")]` を使えない。
- 共有プリミティブは `ArcShared`/`ToolboxMutex` を使用。
- 1ファイル1型/`tests.rs` 配置/日本語ドキュメント。
- fraktor-actor core 依存は必要最小限に留める（streams/core の独立性を維持）。
- fraktor-actor core から streams/core への依存は禁止。
- streams 公開 API に fraktor-actor の型を露出しない。

## 4. 実装アプローチの選択肢

### Option A: `modules/streams` を再構築（core/std に ActorMaterializer を実装）
**概要**: 既存の streams クレートを起点に core を定義し、std に ActorMaterializer/DriveActor を追加  
**利点**: 要件と整合しやすく、既存構造に素直に合致  
**欠点**: ActorSystem 依存を std に閉じる設計が前提、設計/実装コストが高い

### Option B: `fraktor-streams-actor-rs` を別クレート化
**概要**: streams core を純粋化し、Actor 統合は別クレートに分離  
**利点**: 依存境界がさらに明確、Materializer 実装の差し替えが容易  
**欠点**: 現行要件（streams std 内の ActorMaterializer）に反するため仕様調整が必要

### Option C: 段階構築（core → DSL → ActorMaterializer）
**概要**: core と DSL を先行で整備し、ActorMaterializer を後続で追加  
**利点**: YAGNI に沿って段階的に構築可能  
**欠点**: 途中段階では実行統合が未完となる

### Option D: streams core が actor/core に依存する設計
**概要**: Materializer の中核ロジックを actor/core の Scheduler/TickDriver/Extension に直接結び付ける  
**利点**: boilerplate が減り、ActorSystem 統合の設計が簡潔  
**欠点**: streams の独立性が下がり、actor/core への依存が固定化される

## 5. 複雑度/リスク評価
- **Effort**: L（1–2週間）  
  - 新規ドメイン設計 + Actor 統合 + テスト/サンプルが必要
- **Risk**: High  
  - 実行モデル（drive）と DSL 合成規約の設計が全体に影響

## 6. Research Needed（設計フェーズ持ち越し）
- Pekko/Akka オペレータ整理（`map`/`flatMapConcat`/Sink 群の最小語義）。
- `StreamDriveActor` と TickDriver/スケジューラの結合方法。
- Materializer の拡張点（ActorSystem extension として登録するか）。
- streams/core が actor/core に依存する場合の API 境界（actor 型の非露出）。
- remote/cluster 有効時のスモーク構成。

## 7. 設計フェーズへの提案
- **推奨検討**: Option A か Option C を前提に、core/std 分離を守って再構築する。  
- **要決定事項**:
  - DSL コンビネータの最小語義（Pekko Operators Index 準拠）
  - ActorMaterializer の登録方式（拡張/手動生成）
  - tokio 依存の整理（削除・例題更新）
