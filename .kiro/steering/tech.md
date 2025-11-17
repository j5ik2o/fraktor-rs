# 技術スタック
> 最終更新: 2025-11-17

## アーキテクチャ
ワークスペースは `modules/utils`（crate: `fraktor-utils-rs`）と `modules/actor`（crate: `fraktor-actor-rs`）の 2 クレート構成です。両クレートとも `core.rs` で no_std ドメインを `#![no_std]` のまま公開し、`std.rs` 以下を `feature = "std"`/`"tokio-executor"` で有効化する 2 階層モジュールになりました。`fraktor-utils-rs::core` が `RuntimeToolbox` / `NoStdToolbox` / `StdToolbox` を提供し、`fraktor-actor-rs::core` が ActorSystem・SystemMailbox・EventStream を no_std で構築、`std` モジュールが Tokio/ホスト固有の Dispatcher・TickDriver・Builder を後掛けします。supervisor/DeathWatch/EventStream は引き続き system mailbox で `SystemMessage` を先行処理します。

## コア技術
- **言語**: Rust 2024 edition（ワークスペース全体で nightly toolchain を既定とし、`core` 側は `#![no_std]` を前提）。
- **フレームワーク / ランタイム**: `fraktor-actor-rs::core` が embassy/裸メタル環境を、`fraktor-actor-rs::std` + `StdToolbox` が Tokio マルチスレッド実行器を担当し、`tokio-executor` feature で TickDriver/Dispatcher/ログ連携をまとめて有効化。
- **同期基盤**: `portable-atomic(+critical-section)` と `spin` による lock-free/lock-based 混在戦略、`ArcShared` 系の共有所有権プリミティブ。
- **Tick Driver / Scheduler**: `TickDriverBootstrap` + `SchedulerTickExecutor` がハードウェア/手動/Tokio driver を共通 API で駆動し、`StdTickDriverConfig::tokio_quickstart*` がホスト側のデフォルト構成を 1 行で提供します。

## 主要ライブラリ
- `portable-atomic` / `portable-atomic-util`: 割り込み安全なアトミック操作と no_std での `Arc` 代替を提供。
- `heapless` と `dashmap`: バックプレッシャを制御する mailbox 容量と、スレッド安全なディスパッチャキャッシュを構築。
- `embassy-{executor,sync,time}`: Cortex-M ターゲット向けの async 実行器／同期プリミティブを Toolbox にブリッジ。
- `tokio`, `tokio-util`, `tokio-condvar`: ホスト環境での Dispatcher 駆動・`ask` Future 回収・待機制御を提供。
- `postcard` / `prost` / `serde`: 低コストなメッセージシリアライズと API 増設時の互換フォーマットを確保。
- `tracing` + `tracing-subscriber`: EventStream/LoggerSubscriber をホストログや RTT へ橋渡し。

## リモーティング / アドレッシング
- **ActorPathParts & Formatter**: `modules/actor/src/core/actor_prim/actor_path/{parts,formatter}.rs` が system 名・guardian・authority(host/port) を保持し、`ActorPath::root()` で `cellactor` ガーディアンを自動注入します。`modules/actor/src/core/actor_prim/actor_selection/resolver.rs` の `ActorSelectionResolver` は `..` を guardian 境界で遮断し、Pekko の相対選択ルールに追従します。
- **RemoteAuthorityManager**: `modules/actor/src/core/system/remote_authority.rs` が `HashMap<String, AuthorityEntry>` を `ToolboxMutex` で包み、`Unresolved/Connected/Quarantine` の状態を no_std でも駆動します。`VecDeque<AnyMessageGeneric<TB>>` に deferred を蓄積し、`try_defer_send` で隔離中の新規送信を拒否、`poll_quarantine_expiration` と `manual_override_to_connected` で復旧を制御します。
- **イベント観測**: Remoting 由来の InvalidAssociation を `handle_invalid_association` へ集約し、EventStream 通知と同期できるようにしています（spec `pekko-compatible-actor-path` に準拠）。

## スケジューラ / Tick Driver
- **コア抽象**: `modules/actor/src/core/scheduler/tick_driver.rs` が `TickDriverBootstrap`・`TickDriverRuntime`・`TickDriverMatrix`・`SchedulerTickExecutor` を提供し、ハードウェア/手動/Tokio driver を単一 API で扱います。
- **Tokio 構成**: `modules/actor/src/std/scheduler/tick.rs` の `StdTickDriverConfig::tokio_quickstart*` と `tokio_with_handle` が `TickDriverConfig<StdToolbox>` を即時生成し、`docs/guides/tick-driver-quickstart.md` が Quickstart/embedded/manual 向けテンプレを管理します。
- **観測**: `modules/actor/src/core/event_stream/tick_driver_snapshot.rs` がアクティブ driver のスナップショットを定義し、`modules/actor/src/std/system/base.rs` の `ActorSystem::tick_driver_snapshot` 経由で UI/監視から参照できます。EventStream へは `EventStreamEvent::TickDriver` と `SchedulerTickMetricsProbe` によるメトリクスが流れます。

## 開発標準
### 型安全性
- `TypedActor`/`BehaviorGeneric` による型付きプロトコルと、Classic API への `into_untyped` 変換ヘルパで段階的移行を想定。
- `reply_to` をペイロードへ埋め込むルールを徹底し、Classic の `sender()` 相当を API から排除しています。

### コード品質
- 各クレートの `#![deny(...)]` で `unwrap/expect`, `todo`, `unimplemented`, 未使用 async などをコンパイルエラー化。
- カスタム Dylint 群 (`mod-file-lint`, `module-wiring-lint`, `type-per-file-lint`, `tests-location-lint`, `use-placement-lint`, `rustdoc-lint`, `cfg-std-forbid-lint`) でモジュール構造, FQCN import, 1 ファイル 1 構造体, テスト配置, `use` 順序, rustdoc 英語 / 他コメント日本語, `core` 側での `#[cfg(feature = "std")]` 使用禁止を機械的に担保。
- rustdoc (`///`, `//!`) は英語、それ以外のコメント・ドキュメントは日本語で記述する運用を徹底。

### テスト
- モジュール単位テストは `<module>/tests.rs` に配置し、公開 API の統合テストは `modules/actor/tests/*.rs` で ActorSystem シナリオ（DeathWatch, Supervisor, EventStream, TickDriver 等）を網羅。
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
- 設計における価値観は "Less is more" と "YAGNI"
- **no_std ファースト**: `fraktor-actor-rs::core`/`fraktor-utils-rs::core` は `#![no_std]` で固定し、`pub mod std` 自体を feature で丸ごと切り替える。`cfg-std-forbid` lint により core 内部での `#[cfg(feature = "std")]` 分岐を禁止し、標準依存コードは `std` モジュールへ隔離。
- **SystemMessage 先行処理**: `Create/Recreate/Failure/Terminated` をユーザメッセージより先に処理することで、Supervisor 戦略と DeathWatch を deterministic に制御。
- **Std ActorSystemBuilder**: `modules/actor/src/std/system/actor_system_builder.rs` が TickDriver/Scheduler 設定を受け取り、`ActorSystem::from_core` に渡す前に TickDriver をブートストラップする。std 側でのビルドフローは必ずこのビルダー経由で行う。
- **Pekko 互換 actor path**: `ActorPathScheme` + `ActorPathFormatter` によって `fraktor://` URI を canonical に生成し、guardian（`cellactor/system|user`）を暗黙付与します。権限情報は `PathAuthority` で host/port を保持し、Typed/Untyped いずれの API でも同じ表現を使用します。
- **Authority 隔離**: `RemoteAuthorityManagerGeneric` が remoting の隔離判定を centralize し、`VecDeque` キューを掃き出してから `Connected` 化します。deadline が過ぎた quarantined authority は `poll_quarantine_expiration` で自動復旧させ、明示解除 API との二段構えで安全側に倒します。
- **FQCN import 原則**: ランタイム内部は `crate::...` で明示的に参照し、prelude はユーザ公開面のみに限定。
- **Classic ではなく Untyped 呼称**: 既存設計では「Classic」ではなく「Untyped」と呼ぶ。Untyped API (`Scheduler`, Classic ActorRef) と Typed API (`TypedScheduler`, `TypedActorRef`) を明確に分離し、新規開発でも Untyped/Typed という語彙を使用する。
- **参照実装からの逆輸入**: protoactor-go / Apache Pekko を参照しつつ、Rust の所有権と `no_std` 制約に合わせた最小 API を優先する。
- **命名規約**:
  - 所有権やライフサイクルを管理する型のみ `*Handle` サフィックスを許容し、`ArcShared` 等を薄く包む共有参照は `*Shared` を用いる。管理対象が複数ハンドルに及ぶ場合は `*HandleSet` / `*Context` 等で「束ね役・制御面」であることを明示し、単なる参照ラッパーでは `Handle` を名乗らない。命名段階で責務の違いが分かるようにし、Scheduler/Dispatcher まわりの API でも同ルールを徹底する。
  - Facadeというサフィックスも安易に使わない。例えばpub traitにFacadeと命名するのはもってのほか。つまりFacadeは内部実装の都合をインターフェイス名に暴露しているのと同じだからだ
  - Manager, Utilなどの責務が曖昧になる命名は避けて、具体的な名前を付けること
- **Tick Driver の追加判断**: 新規 driver / executor を導入する際は `modules/actor/src/core/scheduler/tick_driver/tick_driver_matrix.rs` にエントリを追加し、`docs/guides/tick-driver-quickstart.md` のテンプレと `StdTickDriverConfig` のヘルパを同期更新する。Code/Doc の両方を更新できない場合は spec 側でギャップを明記する。

---
_スタックと標準を要約し、詳細な API 仕様は各クレートの rustdoc / guides へ委譲します。_
