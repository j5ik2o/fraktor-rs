# fraktor-rs プロジェクト構造

## プロジェクト概要

fraktor-rs は、Akka/Pekko互換のアクターランタイムをRust/no_stdで実装することを目的とした実験的なプロジェクトです。protoactor-goとpekkoのアーキテクチャを参考にしながら、Rustのイディオムに落とし込んでいます。

### 主な特徴

- **no_std対応**: 組込み環境（thumb ターゲット等）での動作をサポート
- **マルチランタイム**: std環境でも動作する共通ユーティリティと actor 実行基盤
- **型安全**: Rust 2024 edition を使用した最新の型システム
- **DeathWatch API**: Akka/Pekko互換の監視APIによる直感的なアクターモデル
- **Typed API**: メッセージ型をコンパイル時に固定できる実験的機能

## ディレクトリ構造

```
fraktor-rs/
├── modules/               # メインモジュール群
│   ├── actor-core/        # アクターランタイムのコア実装 (no_std)
│   ├── actor-std/         # std環境用のヘルパー
│   ├── utils-core/        # コアユーティリティ (no_std)
│   └── utils-std/         # std環境用のユーティリティ
├── docs/                  # プロジェクトドキュメント
│   ├── guides/            # ユーザーガイド
│   ├── old-*/            # 過去の仕様書（アーカイブ）
│   ├── spec.md           # 現在の仕様書
│   └── mailbox-spec.md   # メールボックス仕様
├── lints/                 # カスタムDylintルール
│   ├── mod-file-lint/
│   ├── module-wiring-lint/
│   ├── type-per-file-lint/
│   ├── tests-location-lint/
│   ├── use-placement-lint/
│   ├── rustdoc-lint/
│   └── cfg-std-forbid-lint/
├── scripts/               # 開発用スクリプト
├── openspec/              # OpenSpec仕様管理
├── claudedocs/            # Claude Code生成ドキュメント
└── references/            # 参考資料

```

## モジュール構成

### 1. actor-core モジュール

**場所**: `modules/actor-core/`
**パッケージ名**: `fraktor-actor-core-rs`
**説明**: アクターランタイムのコア実装（no_std対応）

#### 主要コンポーネント

- **actor_prim**: アクタープリミティブ
- **dead_letter**: デッドレター処理
- **dispatcher**: メッセージディスパッチ
- **error**: エラー型定義
- **event_stream**: イベントストリーム
- **futures**: Future実装
- **lifecycle**: ライフサイクル管理
- **logging**: ロギング機能
- **mailbox**: メールボックス実装
- **messaging**: メッセージング
- **props**: アクタープロパティ
- **spawn**: アクター生成
- **supervision**: 監督戦略
- **system**: アクターシステム
- **typed**: 型付きAPI（実験的）

#### 機能フラグ

- `alloc`: アロケータ機能の有効化
- `alloc-metrics`: アロケーションメトリクスの有効化

#### 主要な依存関係

- `fraktor-utils-core-rs`: コアユーティリティ
- `async-trait`: 非同期トレイト
- `heapless`: no_stdコレクション
- `portable-atomic`: ポータブルアトミック操作
- `spin`: スピンロック

### 2. actor-std モジュール

**場所**: `modules/actor-std/`
**パッケージ名**: `fraktor-actor-std-rs`
**説明**: std環境用のヘルパー実装

#### 機能フラグ

- `tokio-executor`: Tokioエグゼキュータサポート

#### 主要な依存関係

- `fraktor-actor-core-rs`: アクターコア
- `fraktor-utils-std-rs`: stdユーティリティ
- `tokio`: Tokioランタイム（オプション）

#### サンプル

- `ping_pong_tokio`: Tokioを使用したping-pongサンプル
- `logger_subscriber`: ロガーサブスクライバーサンプル
- `supervision`: 監督戦略サンプル
- `deadletter`: デッドレターサンプル
- `named_actor`: 名前付きアクターサンプル

### 3. utils-core モジュール

**場所**: `modules/utils-core/`
**パッケージ名**: `fraktor-utils-core-rs`
**説明**: no_std対応の共通ユーティリティ

#### 主要コンポーネント

- **collections**: コレクション型
- **concurrent**: 並行処理プリミティブ
- **runtime_toolbox**: ランタイムツールボックス
- **sync**: 同期プリミティブ
- **timing**: タイミング制御

#### 機能フラグ

- `alloc`: アロケータ機能
- `unsize`: Unsizeトレイトサポート
- `interrupt-cortex-m`: Cortex-M割り込みサポート
- `force-portable-arc`: ポータブルArcの強制使用

#### 主要な依存関係

- `async-trait`: 非同期トレイト
- `spin`: スピンロック
- `portable-atomic`: ポータブルアトミック操作
- `critical-section`: クリティカルセクション

### 4. utils-std モジュール

**場所**: `modules/utils-std/`
**パッケージ名**: `fraktor-utils-std-rs`
**説明**: std環境用のユーティリティ

#### 主要な依存関係

- `fraktor-utils-core-rs`: コアユーティリティ

## 開発ツール

### Dylintカスタムルール

プロジェクトでは、以下のカスタムDylintルールを使用してコード品質を維持しています。

1. **mod-file-lint**: `mod.rs`の使用を禁止
2. **module-wiring-lint**: モジュール配線の検証
3. **type-per-file-lint**: 1ファイル1型の原則を強制
4. **tests-location-lint**: テストの配置場所を検証
5. **use-placement-lint**: use文の配置を検証
6. **rustdoc-lint**: rustdocコメントの品質チェック
7. **cfg-std-forbid-lint**: ランタイム本体での`#[cfg(feature = "std")]`使用を禁止

### ビルドツール

- **Rust版**: 2024 edition
- **Toolchain**: nightly（標準）
- **フォーマッター**: rustfmt（nightly版）
- **ビルドシステム**: Cargo workspace

## ドキュメント

### ガイド

- **module_wiring.md**: モジュール配線ガイド
- **death_watch_migration.md**: DeathWatch APIマイグレーションガイド
- **actor-system.md**: アクターシステムガイド

### 仕様書

- **spec.md**: 現在のプロジェクト仕様
- **mailbox-spec.md**: メールボックス仕様
- **old-*/**: 過去の仕様書（アーカイブ）

## 開発規約

### コーディング規約

1. **言語**: 応対は日本語、rustdocは英語、その他コメント・ドキュメントは日本語
2. **モジュール構成**: `mod.rs`禁止、Rust 2018以降のモジュール構成を採用
3. **型定義**: 1ファイルに複数構造体や複数traitを定義しない
4. **テスト配置**: 対象ファイルと同階層の`hoge/tests.rs`に配置
5. **feature分岐**: ランタイム本体で`#[cfg(feature = "std")]`による分岐は行わない（テストコード内のみ許容）
6. **破壊的変更**: 許容され、最適な設計を優先

### フォーマット

- rustfmt設定は`rustfmt.toml`で管理
- `cargo fmt`はnightly toolchain経由で実行

## ライセンス

MIT OR Apache-2.0
