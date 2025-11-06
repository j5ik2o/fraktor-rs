# cellactor-rs ドキュメント

このディレクトリには、cellactor-rsプロジェクトの包括的なドキュメントが含まれています。

## ドキュメント一覧

### 📚 入門ガイド

- **[使用開始ガイド](./getting-started.md)**
  - cellactor-rsの基本的な使い方
  - 最初のアクターの作成方法
  - メッセージングパターン
  - 子アクターの管理
  - DeathWatch API
  - 監督戦略
  - Typed API（実験的）
  - 実用的なサンプル
  - トラブルシューティング

### 🔧 リファレンス

- **[APIリファレンス](./api-reference.md)**
  - Actor トレイト
  - ActorContext
  - ActorSystem
  - Pid（Process Identifier）
  - SystemMessage
  - SupervisorStrategy
  - Typed API
  - DeathWatch API
  - ライフサイクル制御
  - エラーハンドリング
  - EventStream

### 🏗️ プロジェクト情報

- **[プロジェクト構造](./project-structure.md)**
  - プロジェクト概要
  - ディレクトリ構造
  - モジュール構成
    - actor-core モジュール
    - actor-std モジュール
    - utils-core モジュール
    - utils-std モジュール
  - 開発ツール
  - ドキュメント
  - 開発規約
  - ライセンス

## その他のドキュメント

### 📖 公式ガイド（docs/guides/）

プロジェクトルートの`docs/guides/`ディレクトリには、以下の追加ガイドがあります。

- **[ActorSystemガイド](../docs/guides/actor-system.md)**
  - ActorSystemの初期化
  - メッセージ送信とreply_toパターン
  - 監督機能と停止フロー
  - 監視とオブザーバビリティ
  - Tokioランタイムとの連携

- **[DeathWatch移行ガイド](../docs/guides/death_watch_migration.md)**
  - Akka/PekkoからcellactorへのDeathWatch API移行方法
  - 基本構文の対応表
  - ベストプラクティス
  - FAQ

- **[モジュール配線ガイド](../docs/guides/module_wiring.md)**
  - モジュールの組織化方法

### 📝 仕様書（docs/）

- **[プロジェクト仕様](../docs/spec.md)**: 現在のプロジェクト仕様
- **[メールボックス仕様](../docs/mailbox-spec.md)**: メールボックスの詳細仕様

### 💻 サンプルコード

#### std環境のサンプル（modules/actor-std/examples/）

- `ping_pong_tokio_std`: Tokioランタイムとの統合例
- `death_watch_std`: DeathWatch APIの使用例
- `logger_subscriber_std`: ログ購読の例
- `named_actor_std`: 名前付きアクターの例
- `supervision_std`: 監督戦略の例
- `dead_letter_std`: デッドレター処理の例
- `behaviors_setup_receive_std`: Behaviorパターンの例
- `behaviors_receive_signal_std`: シグナル処理の例
- `behaviors_counter_typed_std`: Typed APIの例

#### no_std環境のサンプル（modules/actor-core/examples/）

- `ping_pong_not_std`: no_std環境での基本的な例
- `ping_pong_typed_not_std`: no_std環境でのTyped APIの例
- `death_watch_no_std`: no_std環境でのDeathWatch例
- `behaviors_setup_receive_no_std`: no_std環境でのBehaviorパターン
- `behaviors_receive_signal_std`: no_std環境でのシグナル処理
- `behaviors_counter_typed_no_std`: no_std環境でのTyped API

## ドキュメントの使い方

### 初めてcellactor-rsを使う方

1. [使用開始ガイド](./getting-started.md)から始めてください
2. 基本的な概念とコード例を学びます
3. [サンプルコード](../modules/actor-std/examples/)で実際の実装例を確認します

### 既存のAkka/Pekkoユーザー

1. [DeathWatch移行ガイド](../docs/guides/death_watch_migration.md)で移行方法を確認
2. [APIリファレンス](./api-reference.md)で詳細なAPI仕様を確認
3. [ActorSystemガイド](../docs/guides/actor-system.md)でActorSystemの違いを理解

### 詳細なAPI仕様が必要な方

1. [APIリファレンス](./api-reference.md)で主要APIの詳細を確認
2. [プロジェクト構造](./project-structure.md)でモジュール構成を理解
3. ソースコードとrustdocコメントを参照

### no_std環境での開発者

1. [使用開始ガイド](./getting-started.md)のno_std環境セクションを確認
2. [プロジェクト構造](./project-structure.md)でno_std対応モジュールを確認
3. `modules/actor-core/examples/`のno_stdサンプルを参照

## ドキュメントの更新

このドキュメントは定期的に更新されます。最新の情報については、プロジェクトのGitHubリポジトリを確認してください。

## フィードバック

ドキュメントに関するフィードバックや改善提案は、GitHubのissueでお知らせください。

## バージョン情報

- **プロジェクトバージョン**: 0.1.0
- **ドキュメント生成日**: 2025-11-06
- **Rust Edition**: 2024
- **対応環境**: no_std / std

## ライセンス

cellactor-rsは、MIT OR Apache-2.0ライセンスの下で提供されています。

---

cellactor-rsプロジェクトへようこそ！質問や提案がありましたら、お気軽にお問い合わせください。
