# ActorSystem Quickstart API 設計提案書

## 1. 現状分析

### 1.1 std (Tokio) 版の初期化フロー

`ping_pong_tokio_std` example を基準とする。

**現在の初期化コード（main 関数のみ、アクター定義を除く）:**

```rust
#[tokio::main(flavor = "multi_thread")]
async fn main() {
    // Step 1: Tokio Handle の取得
    let handle = Handle::current();

    // Step 2: DispatcherConfig の構築（executor のラップ）
    let dispatcher: DispatcherConfig =
        DispatcherConfig::from_executor(
            ArcShared::new(StdSyncMutex::new(Box::new(TokioExecutor::new(handle))))
        );

    // Step 3: Guardian 用 Props の構築（dispatcher を注入）
    let props: Props = Props::from_fn({
        let dispatcher = dispatcher.clone();
        move || GuardianActor::new(dispatcher.clone())
    })
    .with_dispatcher(dispatcher.clone());

    // Step 4: TickDriverConfig の構築
    let tick_driver = TickDriverConfig::tokio_quickstart();

    // Step 5: ActorSystem の生成
    let system = ActorSystem::new(&props, tick_driver).expect("system");

    // Step 6: メッセージ送信
    system.user_guardian_ref().tell(AnyMessage::new(Start)).expect("start");

    // Step 7: 待機
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Step 8: 終了
    system.terminate().expect("terminate");
    ActorFutureListener::new(system.when_terminated()).await;
}
```

**問題点:**

| ステップ | 行数 | 問題 |
|---------|------|------|
| DispatcherConfig 構築 | 3行 | ArcShared/StdSyncMutex/Box のネスト、ユーザーに見えるべきでない内部構造 |
| GuardianActor の dispatcher 依存 | ~15行 | Guardian がユーザーの関心事でない場合に不要な複雑さ |
| TickDriverConfig | 1行 | `tokio_quickstart()` により既に簡略化済み |
| import 文 | ~14行 | 内部型（TokioExecutor, StdSyncMutex 等）の直接参照 |

**main 関数のボイラープレート行数: 約 18行**（アクター定義を除く）
**GuardianActor ボイラープレート: 約 16行**（dispatcher を子に伝播するためだけの型定義）
**import 文: 約 14行**

合計: **約 48行のボイラープレート**（アクター定義を除く初期化関連コード）

### 1.2 no_std 版の初期化フロー

`ping_pong_not_std` example を基準とする。

```rust
fn main() {
    let props = Props::from_fn(|| GuardianActor);
    let (tick_driver, _pulse_handle) = no_std_tick_driver_support::hardware_tick_driver_config();
    let system = ActorSystem::new(&props, tick_driver).expect("system");
    // ...
}
```

**no_std 版の特徴:**
- DispatcherConfig の明示的構築が不要（InlineExecutor がデフォルト）
- `hardware_tick_driver_config()` は example 側のヘルパーであり、ライブラリ側には存在しない
- Guardian に dispatcher を渡す必要がないため、比較的シンプル

**main 関数のボイラープレート: 約 8行**（終了待機含む）

### 1.3 Typed API 版の初期化フロー

`ping_pong_typed_not_std` / `behaviors_counter_typed_std` を基準とする。

```rust
fn main() {
    let props = TypedProps::from_behavior_factory(|| counter(0));
    let (tick_driver, _pulse_handle) = std_tick_driver_support::hardware_tick_driver_config();
    let system = TypedActorSystem::new(&props, tick_driver).expect("system");
    // ...
}
```

Typed 版は Untyped 版と同じ初期化パターン。差異はなし。

### 1.4 Pekko との比較

```scala
// Pekko: 最小構成
val system = ActorSystem.create("my-system")

// Pekko: 名前付き
val system = ActorSystem.create("my-system")

// Pekko: 設定付き
val system = ActorSystem.create("my-system", config)
```

Pekko では:
- ActorSystem 生成時に guardian を指定しない（後から `system.actorOf(props)` でアクターを生成）
- デフォルト設定が豊富（dispatcher, scheduler 等すべて内蔵）
- 設定は Typesafe Config (HOCON) で外部化

**fraktor-rs との構造的差異:**
- fraktor-rs は user guardian を ActorSystem 生成時に必須としている（protoactor-go 由来の設計）
- TickDriverConfig は no_std 対応のため、完全なデフォルトが難しい（ハードウェアタイマー要件）
- std 版では `tokio_quickstart()` が既にデフォルト TickDriver を提供している

---

## 2. 設計案

### 2.1 設計方針

1. **std 版に限定した quickstart API を提供**（no_std 版はハードウェア依存が大きく、汎用デフォルトが困難）
2. **既存の `ActorSystem::new` は変更しない**（quickstart は追加の便利メソッド）
3. **Guardian Props の構築を隠蔽する場合と明示する場合の両方をサポート**
4. **DispatcherConfig の自動構築**（Tokio Handle から自動検出）
5. **YAGNI: 最小限の API 表面積**

### 2.2 API シグネチャ案

#### 案A: `ActorSystem::quickstart` (推奨)

```rust
// modules/actor/src/std/system/base.rs に追加

impl ActorSystem {
    /// Tokio 環境で最小構成の ActorSystem を生成する。
    ///
    /// 内部で以下を自動構築する:
    /// - TickDriverConfig (tokio_quickstart, 10ms resolution)
    /// - DispatcherConfig (現在の Tokio Handle から自動検出)
    ///
    /// # Panics
    ///
    /// Tokio ランタイムのコンテキスト外で呼び出した場合にパニックする。
    pub fn quickstart(props: &Props) -> Result<Self, SpawnError> {
        let tick_driver = TickDriverConfig::tokio_quickstart();
        let dispatcher = DispatcherConfig::tokio_auto();
        let config = ActorSystemConfig::default()
            .with_tick_driver_config(tick_driver)
            .with_default_dispatcher_config(dispatcher);
        Self::new_with_config(props, &config)
    }

    /// カスタマイズ可能な quickstart。
    ///
    /// `configure` クロージャで ActorSystemConfig を調整できる。
    pub fn quickstart_with<F>(props: &Props, configure: F) -> Result<Self, SpawnError>
    where
        F: FnOnce(ActorSystemConfig) -> ActorSystemConfig,
    {
        let tick_driver = TickDriverConfig::tokio_quickstart();
        let dispatcher = DispatcherConfig::tokio_auto();
        let base = ActorSystemConfig::default()
            .with_tick_driver_config(tick_driver)
            .with_default_dispatcher_config(dispatcher);
        let config = configure(base);
        Self::new_with_config(props, &config)
    }
}
```

#### 案B: `TypedActorSystem::quickstart` (Typed API 版)

```rust
impl<M> TypedActorSystem<M>
where
    M: Send + Sync + 'static,
{
    /// Typed API 向け quickstart。
    pub fn quickstart(props: &TypedProps<M>) -> Result<Self, SpawnError> {
        // ActorSystem::quickstart と同等の内部ロジック
    }
}
```

#### 必要な追加 API: `DispatcherConfig::tokio_auto`

```rust
// modules/actor/src/std/dispatch/dispatcher/dispatcher_config.rs に追加

impl DispatcherConfig {
    /// 現在の Tokio Handle から自動的に DispatcherConfig を構築する。
    ///
    /// # Panics
    ///
    /// Tokio ランタイムのコンテキスト外で呼び出した場合にパニックする。
    pub fn tokio_auto() -> Self {
        let handle = Handle::try_current()
            .expect("Tokio runtime handle unavailable");
        Self::from_executor(
            ArcShared::new(StdSyncMutex::new(Box::new(TokioExecutor::new(handle))))
        )
    }
}
```

### 2.3 デフォルト構成の内容

| コンポーネント | デフォルト値 | 根拠 |
|---------------|-------------|------|
| TickDriver | `tokio_quickstart()` (10ms resolution) | 既存の標準ヘルパー |
| Dispatcher | Tokio Handle ベースの TokioExecutor | std 環境では最も一般的 |
| SchedulerConfig | `SchedulerConfig::default()` | 既存のデフォルト値 |
| SystemName | `"default"` | Pekko の慣例に準拠 |
| GuardianKind | `User` | 既存のデフォルト値 |

### 2.4 カスタマイズポイント

`quickstart_with` を使用して以下をオーバーライド可能:

```rust
// 例: システム名とスケジューラ解像度のカスタマイズ
let system = ActorSystem::quickstart_with(&props, |config| {
    config
        .with_system_name("my-actor-system")
        .with_tick_driver_config(
            TickDriverConfig::tokio_quickstart_with_resolution(Duration::from_millis(5))
        )
})?;
```

---

## 3. 実装方針

### 3.1 変更対象ファイル

| ファイル | 変更内容 |
|---------|---------|
| `modules/actor/src/std/system/base.rs` | `quickstart`, `quickstart_with` メソッド追加 |
| `modules/actor/src/std/dispatch/dispatcher/dispatcher_config.rs` | `tokio_auto` メソッド追加 |
| `modules/actor/src/std/typed/system.rs` | Typed 版 `quickstart`, `quickstart_with` 追加 |

### 3.2 既存 API との関係

```
ActorSystem::quickstart(&props)           -- 最も簡単（新規追加）
    |
    v  (内部で呼び出す)
ActorSystem::new_with_config(&props, &config) -- 既存 API（変更なし）
    |
    v  (内部で呼び出す)
CoreActorSystemGeneric::new_with_config()     -- core 層（変更なし）
```

- 既存の `new` / `new_with_config` は一切変更しない
- `quickstart` は `new_with_config` の便利ラッパーに過ぎない
- core 層 (no_std) への変更は不要

### 3.3 no_std 版について

no_std 版は quickstart API を提供しない。理由:

1. TickDriver が完全にハードウェア依存（タイマー割り込み、カスタムパルスソース等）
2. デフォルトの executor が存在しない（環境ごとに異なる）
3. 現在の `Props::from_fn(|| MyActor)` + `ActorSystem::new(&props, tick_driver)` パターンで十分シンプル（DispatcherConfig の構築が不要）

ただし、examples 側の `no_std_tick_driver_support.rs` / `std_tick_driver_support.rs` を参考ヘルパーとしてドキュメントで案内することは有効。

### 3.4 Props の簡易構築について

現状の `Props::from_fn(|| MyActor)` は十分シンプルであり、追加の簡略化は不要。

ただし、std 版で dispatcher を Props に注入するパターン（`props.with_dispatcher(dispatcher)`）については、`ActorSystemConfig::with_default_dispatcher_config` にデフォルト dispatcher を設定することで、個別 Props への注入を不要にできる。これは `quickstart` 内部で自動的に行われる。

---

## 4. Before/After コード比較

### 4.1 std (Tokio) 版: Untyped API

**Before (現在: main 関数 約 18行 + import 14行)**

```rust
use std::{string::String, time::Duration};
use fraktor_actor_rs::{
    core::error::ActorError,
    std::{
        actor::{Actor, ActorContext, ActorRef},
        dispatch::dispatcher::dispatch_executor::TokioExecutor,
        futures::ActorFutureListener,
        messaging::{AnyMessage, AnyMessageView},
        props::Props,
        dispatch::dispatcher::DispatcherConfig,
        system::ActorSystem,
    },
};
use fraktor_utils_rs::{core::sync::ArcShared, std::StdSyncMutex};
use tokio::runtime::Handle;

// ... (GuardianActor は dispatcher を受け取る必要がある) ...

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let handle = Handle::current();
    let dispatcher: DispatcherConfig =
        DispatcherConfig::from_executor(
            ArcShared::new(StdSyncMutex::new(Box::new(TokioExecutor::new(handle))))
        );

    let props: Props = Props::from_fn({
        let dispatcher = dispatcher.clone();
        move || GuardianActor::new(dispatcher.clone())
    })
    .with_dispatcher(dispatcher.clone());

    let tick_driver = TickDriverConfig::tokio_quickstart();
    let system = ActorSystem::new(&props, tick_driver).expect("system");

    system.user_guardian_ref().tell(AnyMessage::new(Start)).expect("start");
    tokio::time::sleep(Duration::from_millis(50)).await;
    system.terminate().expect("terminate");
    ActorFutureListener::new(system.when_terminated()).await;
}
```

**After (改善後: main 関数 約 8行 + import 8行)**

```rust
use std::time::Duration;
use fraktor_actor_rs::{
    core::error::ActorError,
    std::{
        actor::{Actor, ActorContext, ActorRef},
        futures::ActorFutureListener,
        messaging::{AnyMessage, AnyMessageView},
        props::Props,
        system::ActorSystem,
    },
};

// ... (GuardianActor は dispatcher を受け取る必要がなくなる) ...

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let props = Props::from_fn(|| GuardianActor);
    let system = ActorSystem::quickstart(&props).expect("system");

    system.user_guardian_ref().tell(AnyMessage::new(Start)).expect("start");
    tokio::time::sleep(Duration::from_millis(50)).await;
    system.terminate().expect("terminate");
    ActorFutureListener::new(system.when_terminated()).await;
}
```

**削減効果:**
- main 関数: 18行 -> 8行（**56% 削減**）
- import 文: 14行 -> 8行（**43% 削減**、TokioExecutor, ArcShared, StdSyncMutex, Handle が不要に）
- GuardianActor: dispatcher フィールドと child_props メソッドが不要（**~16行削減**）
- 不要な import: `TokioExecutor`, `ArcShared`, `StdSyncMutex`, `Handle`, `DispatcherConfig`

### 4.2 std (Tokio) 版: Typed API

**Before:**

```rust
let props = TypedProps::from_behavior_factory(|| counter(0));
let (tick_driver, _pulse_handle) = std_tick_driver_support::hardware_tick_driver_config();
let system = TypedActorSystem::new(&props, tick_driver).expect("system");
```

**After:**

```rust
let props = TypedProps::from_behavior_factory(|| counter(0));
let system = TypedActorSystem::quickstart(&props).expect("system");
```

**削減効果:** 3行 -> 2行（TickDriver ヘルパーの参照が不要に）

### 4.3 カスタム構成の場合

```rust
let system = ActorSystem::quickstart_with(&props, |config| {
    config
        .with_system_name("my-system")
        .with_tick_driver_config(
            TickDriverConfig::tokio_quickstart_with_resolution(Duration::from_millis(5))
        )
}).expect("system");
```

---

## 5. 設計判断の根拠

### 5.1 なぜ Builder パターンではなく `quickstart` メソッドか

- Builder パターン (`ActorSystemBuilder`) は型の追加が必要で、YAGNI に反する
- `quickstart` は既存の `new_with_config` への薄いラッパーであり、最小限の追加
- カスタマイズは `quickstart_with` のクロージャで対応可能
- 既存の `ActorSystemConfig` が Builder の役割を既に果たしている

### 5.2 なぜ `DispatcherConfig::tokio_auto` を追加するか

- 現在の `DispatcherConfig::from_executor(ArcShared::new(StdSyncMutex::new(Box::new(TokioExecutor::new(handle)))))` は内部実装の詳細がユーザーに漏れている
- `TickDriverConfig::tokio_quickstart()` と同じ抽象度で `DispatcherConfig::tokio_auto()` を提供する
- `tokio_auto` という命名は既存の `TickDriverConfig::tokio_auto` との一貫性を保つ

### 5.3 なぜ no_std 版は対象外か

- no_std 環境ではタイマーハードウェアが環境ごとに異なる
- 汎用的なデフォルトを提供することが困難
- 現在の初期化パターン（Props + TickDriverConfig）は既にシンプル（DispatcherConfig が不要）
- example ヘルパー (`hardware_tick_driver_config`) のドキュメント充実で対応

### 5.4 Props に `with_dispatcher` が不要になる理由

`ActorSystemConfig::with_default_dispatcher_config` に Tokio dispatcher を設定すると、Props で dispatcher を指定しないアクターは自動的にこのデフォルトを使用する。これにより:

- Guardian アクターが子アクターに dispatcher を伝播する必要がなくなる
- 各 Props に `.with_dispatcher(dispatcher.clone())` を付ける必要がなくなる
- アプリケーションコードが dispatcher の存在を意識しなくて済む

---

## 6. 未検討事項（将来の拡張候補）

以下は本提案のスコープ外だが、将来的に検討する価値がある:

1. **`ActorSystem::run_until_terminated` の std 版ラッパー**: 終了待機パターンの簡略化
2. **Prelude モジュール**: `fraktor_actor_rs::std::prelude::*` で頻出型を一括 import
3. **Example の更新**: quickstart API を使用した新しいサンプルの追加
4. **README の "5分クイックスタート" セクション**: quickstart API を使用した最小サンプル

---

## 7. 行数の目標達成度

| 指標 | Before | After | 削減率 |
|------|--------|-------|--------|
| main 関数（std Tokio） | ~18行 | ~8行 | 56% |
| import 文 | ~14行 | ~8行 | 43% |
| Guardian ボイラープレート | ~16行 | 0行 | 100% |
| **合計ボイラープレート** | **~48行** | **~16行** | **67%** |

最も複雑な std/Tokio ケースで約 67% のボイラープレート削減を達成する。
no_std 版は元々シンプルであり、変更の必要がない。

---

## 8. Codex Architect レビュー結果

**判定: 採用してよい設計**

### 実装時の必須対応

| 重要度 | 指摘 | 対応 |
|--------|------|------|
| **高** | `quickstart` / `quickstart_with` に `#[cfg(feature = "tokio-executor")]` が必要 | feature gate を付与 |
| **中** | `DispatcherConfig::tokio_auto` の panic 契約を `# Panics` セクションで明文化 | `TickDriverConfig::tokio_quickstart` と同じ形式で記載 |

### 将来検討

| 重要度 | 指摘 | 対応 |
|--------|------|------|
| **低** | `TypedActorSystem` に std 側の `new_with_config` がない | Typed quickstart は別タスクで検討 |
| **低** | `tokio_auto` 命名は `TickDriverConfig::tokio_auto` と一致しており妥当 | 対応不要 |

### 工数見積

- 必須対応（feature gate + panic 契約 + テスト）: 0.5〜1.0日
- Typed 側拡張（任意）: +0.5日
