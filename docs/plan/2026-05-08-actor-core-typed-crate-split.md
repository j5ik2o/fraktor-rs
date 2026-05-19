# タスク計画

## 元の要求

# タスク指示書: `actor-core` の `typed` 分離と `actor-core-typed` crate 移設

`modules/actor-core/src/core/typed` を `modules/actor-core` から分離し、新 crate `modules/actor-core-typed` へ移設する。最終的な依存方向は `fraktor-actor-core-typed-rs` が `fraktor-actor-core-rs` に依存し、`actor-core` は kernel / untyped runtime、`actor-core-typed` は typed facade / DSL / receptionist / pubsub / delivery を提供する構成にする。

確定スコープは、`actor-core` から `typed` への逆依存除去、`modules/actor-core-typed` 追加、`modules/actor-core/src/core/typed` 配下の移設、downstream import / dependency の新 crate 参照への更新。明示制約として、crate/module ディレクトリ名は `modules/actor-core-typed`、破壊的変更可、deprecated alias / compatibility fallback なし、`actor-core` から typed re-export なし。

## 分析結果

### 目的

`actor-core` を typed API に依存しない kernel / untyped runtime crate に戻し、typed facade と typed 拡張群を新 crate `fraktor-actor-core-typed-rs` に移す。workspace 内の利用側は `actor-core` 経由の `core::typed` import ではなく、新 crate を直接参照する。

### 分解した要件

| # | 要件 | 種別 | 備考 |
|---|------|------|------|
| 1 | `modules/actor-core/src/core.rs` から `pub mod typed;` を削除する | 明示 | `src/core/mod.rs` は存在せず、現行は `src/core.rs` |
| 2 | `modules/actor-core/src/core/typed/**` の production code を新 crate に移す | 明示 | 旧 module / forwarding module は残さない |
| 3 | `modules/actor-core-typed` を workspace member に追加する | 明示 | package 名は既存規約に合わせ `fraktor-actor-core-typed-rs` |
| 4 | `actor-core-typed` が `fraktor-actor-core-rs` に依存する | 明示 | 逆方向は禁止 |
| 5 | `actor-core` から `actor-core-typed` への通常依存を追加しない | 明示 | `Cargo.toml` で依存方向を検証する |
| 6 | `actor-core` から `actor-core-typed` への dev-dependency も追加しない | 暗黙 | actor-core が typed crate を参照しない完了条件から導出 |
| 7 | `kernel` 配下の `core::typed` / typed 型参照を除去する | 明示 | `base.rs`、`actor_cell.rs`、`actor_cell_state.rs` が主要対象 |
| 8 | `ActorSystem` の `TypedActorSystemConfig` 保持を kernel-neutral な設定型に置き換える | 明示 | 現行 `base.rs` は `settings: TypedActorSystemConfig` |
| 9 | typed bootstrap である Receptionist 初期化を `actor-core-typed` 側へ移す | 明示 | 現行 `ActorSystem::bootstrap` 内で receptionist を spawn |
| 10 | message adapter の lifecycle / handle / cleanup を core 側の neutral module へ移す | 明示 | 現行 `ActorCellState` が typed の `AdapterRefHandle` を保持 |
| 11 | `ActorRefResolver` は typed crate 側に置き、core は typed に依存しない解決経路だけを持つ | 明示 | 現行 resolver は typed ref も扱うため typed crate 側が適切 |
| 12 | downstream crate / showcase / test の import を新 crate 参照へ更新する | 明示 | `fraktor_actor_core_rs::core::typed::*` を残さない |
| 13 | typed 利用側 `Cargo.toml` に `fraktor-actor-core-typed-rs` dependency を追加する | 明示 | import だけの変更は禁止 |
| 14 | deprecated alias / fallback / re-export / empty module / `#[path]` / `include!` を追加しない | 明示 | 破壊的変更として扱う |
| 15 | 移設後の test helper 依存を新 crate から到達可能な public test-support へ置き換える | 暗黙 | 旧 `#[cfg(test)]` core 内 helper は別 crate から使えない |

### 参照資料の調査結果

参照資料のうち `modules/actor-core/src/core/mod.rs` は存在せず、実体は `modules/actor-core/src/core.rs` だった。同ファイルは `pub mod kernel;` と `pub mod typed;` を公開しているため、移設時は `pub mod typed;` を削除する。

`modules/actor-core/src/core/typed.rs` は typed API の集約 module で、`actor`、`delivery`、`dsl`、`eventstream`、`message_adapter`、`pubsub`、`receptionist` を公開し、`TypedActorSystem`、`TypedProps`、`TypedActorRef`、`ActorRefResolver` などを re-export している。新 crate の `src/lib.rs` はこの公開面を crate root に移し、利用側は `fraktor_actor_core_typed_rs::{TypedActorSystem, TypedProps, dsl::Behaviors, receptionist::Receptionist}` のように直接 import する形にする。

`modules/actor-core/src/core/kernel/system/base.rs` は `TypedActorSystemConfig`、`TypedProps`、`ActorRefResolver`、`Receptionist` を直接 import しており、`ActorSystem` が typed config を保持し、bootstrap 内で typed receptionist を spawn し、作成時に resolver を install している。ここが逆依存の最大箇所。

`modules/actor-core/src/core/kernel/actor/actor_cell.rs` と `actor_cell_state.rs` は `typed::message_adapter::{AdapterLifecycleState, AdapterRefHandle, AdapterRefHandleId}` に依存している。adapter cleanup は runtime 責務なので、handle / lifecycle / id は core の neutral module に移し、typed crate には adapter sender / payload / registry など typed 固有部分を残す。

`modules/stream-core/src/core/dsl/topic_pub_sub.rs` は `fraktor_actor_core_rs::core::typed::{Behavior, TypedActorRef, TypedProps, dsl::Behaviors, pubsub::{Topic, TopicCommand}}` を使っている。`modules/stream-core/Cargo.toml` は現在 `fraktor-actor-core-rs` のみ依存しているため、新 crate dependency の追加が必要。

showcase / legacy / actor-core tests に typed import が多数ある。特に `modules/actor-core/tests/typed_scheduler.rs` と `typed_user_flow_e2e.rs` は actor-core 側に残すと actor-core が typed crate に dev-dependency する形になるため、新 crate の tests へ移す。

### スコープ

影響範囲は以下。

- workspace root `Cargo.toml` と `Cargo.lock`
- `modules/actor-core/Cargo.toml`
- `modules/actor-core/src/lib.rs`
- `modules/actor-core/src/core.rs`
- `modules/actor-core/src/core/kernel/**`
- `modules/actor-core/src/core/typed.rs`
- `modules/actor-core/src/core/typed/**`
- 新規 `modules/actor-core-typed/Cargo.toml`
- 新規 `modules/actor-core-typed/src/**`
- 新規 `modules/actor-core-typed/tests/**`
- `modules/stream-core/Cargo.toml`
- `modules/stream-core/src/core/dsl/topic_pub_sub.rs`
- `modules/stream-core/src/core/dsl/topic_pub_sub/tests.rs`
- `showcases/std/Cargo.toml`
- `showcases/std/src/lib.rs`
- `showcases/std/typed/**`
- `showcases/std/legacy/**` の typed API import 箇所

### 検討したアプローチ

| アプローチ | 採否 | 理由 |
|-----------|------|------|
| `actor-core` に `typed` re-export を残す | 不採用 | 明示制約違反。downstream 修正を回避する compatibility shim になる |
| `#[path]` / `include!` で typed tree を新 crate から参照する | 不採用 | 明示制約違反。crate 分割ではなく配置回避になる |
| `actor-core` に `actor-core-typed` dev-dependency を追加して既存 typed tests を残す | 不採用 | `actor-core` が typed crate を参照しない完了条件に反する |
| typed API を `fraktor_actor_core_typed_rs::core::typed::*` にする | 不採用 | 旧構造を温存するだけで、新 crate の公開面として冗長 |
| typed API を crate root 直下に公開する | 採用 | 旧 `core::typed` の公開 API を新 crate の責務として素直に提供できる |
| `ActorRefResolver` を core に移す | 不採用 | 現行 resolver は `TypedActorRef` も扱うため typed crate 側に置く方が依存方向を守れる |
| message adapter handle / lifecycle を core に移す | 採用 | `ActorCell` の cleanup は runtime 責務であり、typed 固有 sender / payload と分離できる |
| Receptionist spawn を `TypedActorSystem::create_from_props` 側へ移す | 採用 | Receptionist は typed bootstrap に属し、core bootstrap から除去できる |
| core に登録付き system actor spawn helper を追加する | 採用 | typed crate から private rollback に触れず、安全に extra top-level 登録できる |

### 実装アプローチ

1. workspace root `Cargo.toml` に `modules/actor-core-typed` を member と workspace dependency として追加する。
2. `modules/actor-core-typed/Cargo.toml` を新規作成し、package 名を `fraktor-actor-core-typed-rs` にする。`fraktor-actor-core-rs`、`fraktor-utils-core-rs`、`futures`、`tracing`、`ahash`、`hashbrown`、`portable-atomic` など、typed tree の実使用に基づく依存を定義する。
3. `modules/actor-core-typed/src/lib.rs` を作り、旧 `modules/actor-core/src/core/typed.rs` の公開 API を crate root に移す。旧 `crate::core::typed::*` import は `crate::*` または crate 内 module 参照へ、旧 `crate::core::kernel::*` import は `fraktor_actor_core_rs::core::kernel::*` へ置き換える。
4. `modules/actor-core/src/core.rs` から `pub mod typed;` を削除し、旧 `modules/actor-core/src/core/typed.rs` と `typed/**` は production 配置として残さない。
5. `ActorSystem` の `TypedActorSystemConfig` を core-neutral な設定型へ置換する。typed 側の `TypedActorSystemConfig` は core settings から生成する wrapper として新 crate に置く。
6. `ActorSystem::bootstrap` から Receptionist spawn と `SYSTEM_RECEPTIONIST_TOP_LEVEL` 登録を削除する。typed 側の `TypedActorSystem::create_from_props` が `ActorSystem::create_from_props_with_init` の init hook で Receptionist を初期化する。
7. `ActorRefResolver::install` は typed crate 側で、core の `ActorSystem` 作成完了後に実行する。config extension installers が先に入る既存順序を壊さないよう、既に resolver extension が登録済みなら上書きしない現行挙動を維持する。
8. `AdapterLifecycleState`、`AdapterRefHandle`、`AdapterRefHandleId` を `modules/actor-core/src/core/kernel/actor/message_adapter` 相当の neutral module に移す。typed の `AdapterRefSender`、`AdapterEnvelope`、`AdapterPayload`、`MessageAdapterRegistry` は new crate に残す。
9. typed registry が必要とする `ActorCell` の adapter handle 取得と mailbox sender 取得について、core 側に最小限の公開 API を追加する。公開範囲は crate 分割後に必要なものだけに限定する。
10. kernel event stream の `TypedUnhandledMessageEvent` は typed 固有名を core に残さないため、neutral な `UnhandledMessageEvent` へ置換する。typed behavior runner からは neutral event を publish する。
11. `modules/actor-core/tests/typed_scheduler.rs` と `typed_user_flow_e2e.rs` を `modules/actor-core-typed/tests/` に移す。`tests/common` の helper は新 crate 側へ移すか、必要最小限を test 内に置く。
12. moved typed tests 内の `ActorSystem::new_empty` / `new_empty_with` と core 内部 `TestTickDriver` 参照は、`fraktor_actor_adaptor_std_rs::std::system::{new_empty_actor_system, new_empty_actor_system_with}` と `fraktor_actor_adaptor_std_rs::std::tick_driver::TestTickDriver` に置き換える。`actor-adaptor-std` は dev-dependency で `test-support` feature を有効にする。
13. `stream-core`、`showcases/std`、typed API を使う tests / examples の `Cargo.toml` に `fraktor-actor-core-typed-rs` を追加し、import を新 crate に更新する。
14. `rg "core::typed|actor_core.*typed|TypedActorSystem|TypedProps|Receptionist|ActorRefResolver"` で旧 import と kernel 側 typed 参照を確認し、実装漏れを潰す。
15. `cargo check --workspace`、`cargo test --workspace`、標準ゲートとして `./scripts/ci-check.sh ai all` を実行対象にする。

### 到達経路・起動条件

| 項目 | 内容 |
|------|------|
| 利用者が到達する入口 | Rust crate API。旧入口は `fraktor_actor_core_rs::core::typed::*`、新入口は `fraktor_actor_core_typed_rs::*` |
| 更新が必要な呼び出し元・配線 | `stream-core`、`showcases/std`、actor-core 内 typed tests の移設先、workspace dependency |
| 起動条件 | feature flag や runtime flag は追加しない。typed API を使う crate が `fraktor-actor-core-typed-rs` に依存して import する |
| 未対応項目 | なし。旧入口は compatibility として残さない |

## 実装ガイドライン

- 参照パターンとして、crate metadata / lint / feature の書き方は `modules/actor-core/Cargo.toml` と `modules/stream-core/Cargo.toml` に合わせる。
- 参照パターンとして、workspace member / workspace dependency の追加は root `Cargo.toml` の既存 `modules/*` crate 群の並びに合わせる。
- `TypedActorSystem::create_from_props` は現行 `modules/actor-core/src/core/typed/system.rs` の `ActorSystem::create_from_props` 呼び出しを、Receptionist 初期化 hook 付きの作成に置き換える。
- `ActorSystem::bootstrap` から typed Receptionist 処理を抜く際、root/user/system guardian の作成順序と rollback は維持する。
- typed crate から system actor を登録するため、core 側には「spawn と extra top-level 登録」をまとめる最小 helper を置く。typed 側で `spawn_system_actor` と `register_extra_top_level` をばらばらに呼んで rollback を欠落させない。
- `ActorRefResolver` は typed crate に置き、core 側には untyped `ActorSystem::resolve_actor_ref` だけを残す。typed resolver が必要な call site は typed crate 内へ寄せる。
- message adapter は、core が保持・cleanup する lifecycle / handle と、typed が message 変換する sender / envelope / registry を分ける。
- `modules/actor-core/src/core/kernel/**` に `typed::`、`TypedActorSystem`、`TypedProps`、`Receptionist`、`ActorRefResolver` が残らないようにする。コメント中の `Receptionist` も検証 grep に引っかかるため、kernel の説明として不要なら削除・言い換える。
- `actor-core` の `Cargo.toml` には `fraktor-actor-core-typed-rs` を dependency / dev-dependency として追加しない。
- `actor-core` に `pub mod typed`、空 module、forwarding module、deprecated alias、compatibility fallback を追加しない。
- `#[path]` / `include!` による crate 分割回避は禁止。
- import 更新だけで終えず、利用側 crate の `Cargo.toml` dependency を必ず追加する。
- moved tests は actor-core 内部 `#[cfg(test)]` helper に依存しない。public test-support crate 経由へ置換する。
- `Cargo.lock` は workspace 構成変更後に更新対象に含める。

## スコープ外

| 項目 | 除外理由 |
|------|---------|
| typed API の互換 alias 提供 | 明示制約で禁止 |
| `actor-core` から typed API の re-export | 明示制約で禁止 |
| typed facade の機能追加 | 今回は crate 分離が目的であり、新機能要求ではない |
| unrelated dead code cleanup | AGENTS 指示の surgical changes に反する |

## 確認事項

なし。実装判断に必要な事項は現行コード調査で解決済み。