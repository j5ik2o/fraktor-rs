# 技術スタック
> 最終更新: 2025-11-12

## アーキテクチャ
`utils-core` → `actor-core` → `actor-std` の三層で構成し、no_std を前提に抽象化した `RuntimeToolbox` を核に、STD 依存の実装は別クレートへ分離します。スーパーバイザ／DeathWatch／EventStream は system mailbox による `SystemMessage` 優先処理で統一し、Akka/Pekko・protoactor-go の語彙を Rust の所有権モデルへマッピングしています。`actor_prim::actor_path` では `ActorPathScheme`（`pekko`, `pekko.tcp`）と `GuardianKind` を分離した `ActorPathParts` を導入し、`ActorPathFormatter` が canonical URI を生成します。

## コア技術
- **言語**: Rust 2024 edition（ワークスペース全体で nightly toolchain を既定とし、`#![no_std]` を前提）。
- **フレームワーク / ランタイム**: Tokió マルチスレッドランタイム（`actor-std` 経由）と `embassy` 系 no_std 実行環境に同一 API で対応。
- **同期基盤**: `portable-atomic(+critical-section)` と `spin` による lock-free/lock-based 混在戦略、`ArcShared` 系の共有所有権プリミティブ。

## 主要ライブラリ
- `portable-atomic` / `portable-atomic-util`: 割り込み安全なアトミック操作と no_std での `Arc` 代替を提供。
- `heapless` と `dashmap`: バックプレッシャを制御する mailbox 容量と、スレッド安全なディスパッチャキャッシュを構築。
- `embassy-{executor,sync,time}`: Cortex-M ターゲット向けの async 実行器／同期プリミティブを Toolbox にブリッジ。
- `tokio`, `tokio-util`, `tokio-condvar`: ホスト環境での Dispatcher 駆動・`ask` Future 回収・待機制御を提供。
- `postcard` / `prost` / `serde`: 低コストなメッセージシリアライズと API 増設時の互換フォーマットを確保。
- `tracing` + `tracing-subscriber`: EventStream/LoggerSubscriber をホストログや RTT へ橋渡し。

## リモーティング / アドレッシング
- **ActorPathParts & Formatter**: `modules/actor-core/src/actor_prim/actor_path/{parts,formatter}.rs` が system 名・guardian・authority(host/port) を保持し、`ActorPath::root()` で `cellactor` ガーディアンを自動注入します。`ActorSelectionResolver` は `..` を guardian 境界で遮断し、Pekko の相対選択ルールに追従します。
- **RemoteAuthorityManager**: `modules/actor-core/src/system/remote_authority.rs` が `HashMap<String, AuthorityEntry>` を `ToolboxMutex` で包み、`Unresolved/Connected/Quarantine` の状態を no_std でも駆動します。`VecDeque<AnyMessageGeneric<TB>>` に deferred を蓄積し、`try_defer_send` で隔離中の新規送信を拒否、`poll_quarantine_expiration` と `manual_override_to_connected` で復旧を制御します。
- **イベント観測**: Remoting 由来の InvalidAssociation を `handle_invalid_association` へ集約し、EventStream 通知と同期できるようにしています（spec `pekko-compatible-actor-path` に準拠）。

## 開発標準
### 型安全性
- `TypedActor`/`BehaviorGeneric` による型付きプロトコルと、Classic API への `into_untyped` 変換ヘルパで段階的移行を想定。
- `reply_to` をペイロードへ埋め込むルールを徹底し、Classic の `sender()` 相当を API から排除しています。

### コード品質
- 各クレートの `#![deny(...)]` で `unwrap/expect`, `todo`, `unimplemented`, 未使用 async などをコンパイルエラー化。
- カスタム Dylint 群 (`mod-file-lint`, `module-wiring-lint`, `type-per-file-lint`, `tests-location-lint`, `use-placement-lint`, `rustdoc-lint`, `cfg-std-forbid-lint`) でモジュール構造, FQCN import, 1 ファイル 1 構造体, テスト配置, `use` 順序, rustdoc 英語 / 他コメント日本語, ランタイムでの `#[cfg(feature = "std")]` 禁止を機械的に担保。
- rustdoc (`///`, `//!`) は英語、それ以外のコメント・ドキュメントは日本語で記述する運用を徹底。

### テスト
- モジュール単位テストは `<module>/tests.rs` に配置し、公開 API の統合テストは `crate/tests/*.rs` で ActorSystem シナリオ（DeathWatch, Supervisor, EventStream 等）を網羅。
- `scripts/ci-check.sh` の `no-std`, `std`, `embedded`, `doc` サブコマンドでターゲット別の検証を自動化し、`THUMB` ターゲット (`thumbv6m`, `thumbv8m.main`) までカバー。

## 開発環境
### 必須ツール
- Rust nightly toolchain（`RUSTUP_TOOLCHAIN` 未設定時は `nightly` を既定）
- `cargo-dylint` と Rust コンポーネント `rustc-dev` / `llvm-tools-preview`（カスタム lint ビルド用）
- `rustup target add thumbv6m-none-eabi thumbv8m.main-none-eabi`（no_std クロスチェック）
- 任意: `Tokio` 実行用のホスト OS ロガー、`embassy` 対応ハードウェア SDK

### よく使うコマンド
```bash
scripts/ci-check.sh lint                 # rustfmt --check
scripts/ci-check.sh dylint module-wiring-lint
scripts/ci-check.sh clippy               # -D warnings をワークスペース一括
scripts/ci-check.sh no-std std embedded  # ターゲット別テスト
scripts/ci-check.sh doc examples test    # ドキュメント・examples・workspace test
scripts/ci-check.sh all                  # CI と同等フルスイート
```

## 重要な技術判断
- **no_std ファースト**: ランタイム本体で `#[cfg(feature = "std")]` を禁止し、標準依存コードは `actor-std`/`utils-std` に隔離。
- **SystemMessage 先行処理**: `Create/Recreate/Failure/Terminated` をユーザメッセージより先に処理することで、Supervisor 戦略と DeathWatch を deterministic に制御。
- **Pekko 互換 actor path**: `ActorPathScheme` + `ActorPathFormatter` によって `pekko://` URI を canonical に生成し、guardian（`cellactor/system|user`）を暗黙付与します。権限情報は `PathAuthority` で host/port を保持し、Typed/Untyped いずれの API でも同じ表現を使用します。
- **Authority 隔離**: `RemoteAuthorityManagerGeneric` が remoting の隔離判定を centralize し、`VecDeque` キューを掃き出してから `Connected` 化します。deadline が過ぎた quarantined authority は `poll_quarantine_expiration` で自動復旧させ、明示解除 API との二段構えで安全側に倒します。
- **FQCN import 原則**: ランタイム内部は `crate::...` で明示的に参照し、prelude はユーザ公開面のみに限定。
- **参照実装からの逆輸入**: protoactor-go / Apache Pekko を参照しつつ、Rust の所有権と `no_std` 制約に合わせた最小 API を優先する。

---
_スタックと標準を要約し、詳細な API 仕様は各クレートの rustdoc / guides へ委譲します。_
