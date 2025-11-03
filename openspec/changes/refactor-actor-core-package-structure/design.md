# Design: Actor-Core Package Structure Refactoring

## 概要
現在フラットに配置されている約107個のモジュールファイルを、機能別に13個の論理的なパッケージに階層化する。参照実装（Pekko/ProtoActor-Go）の構造に倣いつつ、Rustのベストプラクティスと Module Wiring ガイドライン（再エクスポートを最小化する運用）に従った設計とする。

## パッケージ構造の詳細

### 1. actor_prim/ - アクタープリミティブ
**責務**: アクターシステムの最も基本的な型とtrait

**含まれるモジュール**:
- `actor.rs` - `Actor` trait定義
- `actor_ref.rs` - アクターへの参照（`ActorRef`, `ActorRefSender`, `AskReplySender`, `NullSender`）
- `actor_cell.rs` - アクターのカプセル化
- `actor_context.rs` - アクター実行コンテキスト
- `pid.rs` - プロセスID
- `child_ref.rs` - 子アクターへの参照
- `receive_state.rs` - メッセージ受信状態
- `actor_ref_internal.rs` など内部実装を分離したファイル

**設計判断**:
- デフォルト構成を `actor_prim/` ディレクトリに再配置し、Rust 2018 のパス表記に従う。
- 内部実装は `*_internal.rs` に切り出し、`pub(crate)` で可視性制御
- 公開APIと内部実装を明確に分離
- ルートファイルでは子モジュールを `pub mod` するだけに留め、再エクスポートを禁止

**公開パス例**:
```rust
use cellactor_core::actor_prim::actor::Actor;
use cellactor_core::actor_prim::actor_ref::ActorRef;
use cellactor_core::actor_prim::actor_cell::ActorCell;
```
ルートファイル `actor_prim.rs` では子モジュールを `pub mod actor;` のように公開し、`pub use` による再エクスポートは行わない。

### 2. messaging/ - メッセージング
**責務**: メッセージの型消去、Ask/Tellパターン、システムメッセージ

**含まれるモジュール**:
- `any_message.rs` - 型消去されたメッセージ
- `any_message_view.rs` - メッセージビュー
- `ask_response.rs` - Ask応答
- `message_invoker.rs` - メッセージ呼び出しとミドルウェア
- `system_message.rs` - システムメッセージ

**設計判断**:
- メッセージ処理に関連する機能を集約
- `AnyMessage` を中心としたメッセージング抽象化

### 3. mailbox/ - メールボックス
**責務**: メッセージキューとその設定・監視

**含まれるモジュール**:
- `mailbox.rs` - メールボックス実装（`Mailbox`, `MailboxInstrumentation`等）
- `capacity.rs` - メールボックス容量設定
- `policy.rs` - メールボックスポリシー
- `overflow_strategy.rs` - オーバーフロー戦略
- `metrics.rs` - メトリクスイベント

**設計判断**:
- 設定関連のファイルを簡潔な名前に変更（`mailbox_capacity.rs` → `capacity.rs`）
- パッケージ内で完結する命名により、可読性向上

### 4. supervision/ - スーパービジョン
**責務**: エラーハンドリングと再起動戦略

**含まれるモジュール**:
- `strategy.rs` - スーパーバイザー戦略（`SupervisorStrategy`, `SupervisorStrategyKind`）
- `directive.rs` - スーパーバイザー指令（`SupervisorDirective`）
- `restart_statistics.rs` - 再起動統計
- `options.rs` - スーパーバイザーオプション

**設計判断**:
- `SupervisorDirective` を独立したモジュールに分離（関心の分離）

### 5. props_ (ディレクトリ) - アクター生成設定
**責務**: アクター生成時の設定（Props）

**含まれるモジュール**:
- `props.rs` - Props構造体
- `factory.rs` - アクターファクトリ（`ActorFactory`）
- `mailbox_config.rs` - メールボックス設定
- `dispatcher_config.rs` - ディスパッチャー設定
- `supervisor_options.rs` - スーパーバイザーオプション（supervision/とは別管理）

**設計判断**:
- 従来の `spawning/` を `props_` と `spawn_` に分離（mod.rs を使わずに表現）
- 設定（Props）と実行（spawn）の責務を明確化

### 6. spawn_ (ディレクトリ) - アクター生成処理
**責務**: アクターの実際の生成処理とエラー

**含まれるモジュール**:
- `spawn_error.rs` - 生成エラー
- `name_registry.rs` - 名前レジストリ
- `name_registry_error.rs` → `spawn/name_registry_error.rs` または `error/name_registry_error.rs`

**設計判断**:
- 生成実行に関する機能を集約
- `props/` とは独立して管理

### 7. system_ (ディレクトリ) - アクターシステム
**責務**: システム全体の管理

**含まれるモジュール**:
- `system.rs` - アクターシステム（`ActorSystemGeneric`, `ActorSystem`）
- `system_state.rs` - システム状態
- `dispatcher.rs` - ディスパッチャー（`Dispatcher`, `DispatchExecutor`, `DispatchHandle`）

**設計判断**:
- システムレベルの抽象化を集約
- ディスパッチャーをシステムパッケージに配置

### 8. eventstream/ - イベントストリーム
**責務**: イベントの発行と購読

**含まれるモジュール**:
- `event_stream.rs` - イベントストリーム本体
- `event.rs` - イベント型（`EventStreamEvent`）
- `subscriber.rs` - サブスクライバー
- `subscriber_entry.rs` - サブスクライバーエントリ
- `subscription.rs` - サブスクリプション

**設計判断**:
- Pekko/ProtoActorに倣いトップレベルパッケージとして配置
- `system/event_stream/` ではなく、独立した `eventstream/` として扱う

### 9. lifecycle_ (ディレクトリ) - ライフサイクル
**責務**: アクターのライフサイクルイベントとステージ管理

**含まれるモジュール**:
- `event.rs` - ライフサイクルイベント
- `stage.rs` - ライフサイクルステージ

### 10. deadletter_ (ディレクトリ) - デッドレター
**責務**: 配信できないメッセージの管理

**含まれるモジュール**:
- `deadletter.rs` - デッドレター処理（`Deadletter`, `DeadletterGeneric`）
- `entry.rs` - デッドレターエントリ
- `reason.rs` - デッドレター理由

### 11. logging_ (ディレクトリ) - ロギング
**責務**: ログイベントとサブスクライバー管理

**含まれるモジュール**:
- `event.rs` - ログイベント
- `level.rs` - ログレベル
- `subscriber.rs` - ログサブスクライバー
- `writer.rs` - ログライター

### 12. futures_ (ディレクトリ) - Future統合
**責務**: 非同期処理とFutureとの統合

**含まれるモジュール**:
- `actor_future.rs` - ActorFuture
- `listener.rs` - Futureリスナー

### 13. error_ (ディレクトリ) - エラー型
**責務**: エラー型の集約

**含まれるモジュール**:
- `actor_error.rs` - アクターエラー
- `actor_error_reason.rs` - エラー理由
- `send_error.rs` - 送信エラー
- `name_registry_error.rs` - 名前レジストリエラー（オプション）

**設計判断**:
- エラー型を一箇所に集約することで、エラーハンドリングの一貫性を向上
- 各パッケージからエラー型を分離し、依存関係を簡素化

## 依存関係の設計

### レイヤー構造
```
Layer 5: system, eventstream, deadletter, logging
         ↑
Layer 4: spawn, props
         ↑
Layer 3: supervision, actor_prim (actor_cell, actor_context)
         ↑
Layer 2: mailbox, messaging, lifecycle
         ↑
Layer 1: actor_prim (primitives), error
         ↑
Layer 0: futures (横断的関心事)
```

### 依存関係の原則
1. **下位レイヤーは上位レイヤーに依存しない**（依存性逆転の原則）
2. **同レイヤー内の依存は最小限に**
3. **循環依存は禁止**
4. **共通の型は下位レイヤーに配置**
5. **再エクスポートは Module Wiring ガイドラインに従い原則禁止**

## 可視性戦略

### pub(crate) の使用
- パッケージ内でのみ使用する型・関数に適用
- 例: `actor_prim/actor_ref_internal.rs` 等の内部ファイル
- ルートファイルは `pub mod` のみを宣言し、`pub use` を使用しない

### internal ディレクトリを使わない理由
- Rust 2018 の方針に従い `mod.rs` を前提とする構成は採用しない。
- 公開 API と内部実装を分ける場合は `*_internal.rs` へ分割し `pub(crate)` で制御する。
- テストや補助コードも同様に、必要な場合は `test_support.rs` などフラットなファイル名で配置する。

### 公開API の原則
- 各パッケージのルートファイル（例: `actor_prim.rs`）は子モジュールを `pub mod` で公開するのみとし、`pub use` を行わない
- ユーザーは `use cellactor_core::actor_prim::actor::Actor;` のように階層パスでインポートする
- ユーザー向けの簡易インポートは `prelude` を通じて提供する（prelude 内の再エクスポートは許容）
- 内部実装の詳細は公開しない

## prelude の設計

### 目的
よく使われる型を集約し、ユーザーの利便性を向上

### 含まれる型
```rust
// Core types
pub use crate::actor::{Actor, ActorRef, ActorCell, ActorContext, Pid, ChildRef};

// Messaging
pub use crate::messaging::{AnyMessage, AskResponse, SystemMessage};

// System
pub use crate::system::{ActorSystemGeneric, ActorSystem};

// Props and Spawning
pub use crate::props::Props;
pub use crate::spawn::SpawnError;

// Supervision
pub use crate::supervision::{SupervisorStrategy, SupervisorDirective};

// Lifecycle
pub use crate::lifecycle::{LifecycleEvent, LifecycleStage};

// Futures
pub use crate::futures::ActorFuture;

// Error types
pub use crate::error::{ActorError, ActorErrorReason, SendError};
```

### 使用例
```rust
use cellactor_core::prelude::*;

// すべての主要な型がインポート済み
struct MyActor;

impl Actor for MyActor {
    // ...
}
```

## マイグレーション計画

### フェーズ1: 構造作成（破壊的変更を含む）
- 新しいディレクトリ作成
- ファイル移動
- ルートファイル（例: `actor_prim.rs`）で `pub mod` 宣言のみを定義
- `lib.rs` からフラットな再エクスポートを削除
- テスト確認

### フェーズ2: 検証
- ビルド検証
- 全テスト実行
- 静的解析
- ドキュメント生成確認

### フェーズ3: ドキュメント整備
- パッケージドキュメント追加
- `prelude.rs` 作成
- マイグレーションガイド作成

## パフォーマンスへの影響

### ビルド時間
- パッケージ分割により並列ビルドの最適化が期待できる
- モジュール間の依存関係が明確になり、インクリメンタルビルドが効率化

### ランタイムパフォーマンス
- 構造変更のみで、実行時のパフォーマンスには影響なし
- インライン化や最適化は従来通り機能

## 参照実装との比較

### Pekko (Scala)
- `actor/` ディレクトリに主要な型を配置
- `dungeon/` で内部実装を隠蔽
- トップレベルにEventStreamを配置

### ProtoActor-Go
- `actor/` パッケージにすべての機能をフラット配置
- ファイル数は多いが、一つのパッケージに集約

### 本実装の方針
- Pekkoの階層化とProtoActorの明確な命名を組み合わせ
- Rustの慣習（prelude、`*_internal.rs`）を適用
- `no_std` 対応を維持しつつ、保守性を向上

## 将来の拡張性

### 新機能の追加
- 新しいパッケージを追加することで、既存機能への影響を最小化
- 例: `cluster/`, `persistence/`, `router/` など

### 機能のオプション化
- 将来的に、パッケージごとに機能フラグを設定可能
- 例: `features = ["logging", "deadletter", "futures"]`

### 型パラメータ化の拡張
- 現在の `RuntimeToolbox` パターンを継続
- パッケージごとに異なる実装を差し替え可能

## リスク評価

### 高リスク
- なし（後方互換性を完全に維持）

### 中リスク
- ファイル移動によるテスト破損 → 段階的な移動とテストで緩和
- マージコンフリクト → 早期実施で緩和

### 低リスク
- ドキュメント不足 → フェーズ3で対応
- 内部可視性の誤設定 → レビューと検証で対応

## まとめ

この設計により、以下のメリットが得られる：
1. **明確な責任分離**: 各パッケージが明確な役割を持つ
2. **保守性の向上**: コードの配置場所が自明になり、変更の影響範囲が限定される
3. **内部実装の保護**: `*_internal.rs` と `pub(crate)` により、APIの境界が明確化
4. **参照実装との整合性**: Pekko/ProtoActorの設計思想を踏襲
5. **拡張性**: 新機能の追加場所が明確で、段階的な機能追加が容易
6. **後方互換性**: 既存コードはすべて動作し続ける
