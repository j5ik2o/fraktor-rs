## Context

現在の dispatcher まわりは、概念としての `Dispatcher` よりも `DispatcherConfig` が主語になっている。

- `DispatchExecutor` は runtime primitive である
- `DispatcherShared` は runtime handle である
- `DispatcherConfig` は設定オブジェクトである
- しかし actor / system の接続点では `DispatcherConfig` が直接使われており、policy 抽象が存在しない

このため、`PinnedDispatcher` は Pekko の policy concept ではなく、`ThreadedExecutor` を包む config factory として表現されている。結果として `XXXDispatcher` family の拡張点がなく、Tokio / embassy など runtime ごとの差分も policy と primitive の 2 層に分離されていない。

本 change は、dispatcher family を trait/provider 中心で再定義し、legacy abstraction を互換ブリッジなしで置き換える。

## Goals / Non-Goals

**Goals:**
- `Dispatcher` 概念を trait として明示する
- `PinnedDispatcher` を dispatcher family の一員として正規化する
- `XXXDispatcher` を追加可能な拡張点を設ける
- `DispatchExecutor` を backend primitive へ後退させるか、不要なら除去する
- actor / system の接続点を policy/provider 中心へ置き換える
- Tokio / embassy への伸びしろを阻害しない

**Non-Goals:**
- mailbox アルゴリズム自体の変更
- dispatcher throughput / starvation の意味論変更
- すべての runtime backend をこの change で同時実装すること
- 後方互換ブリッジの維持

## Target Model

完成形の責務分割は以下とする。

```text
core
  Dispatcher              // actor mailbox を駆動する公開抽象
  DispatcherProvider      // Dispatcher を provision する公開抽象
  DispatcherSettings      // throughput / starvation 等の設定値
  Dispatchers             // id -> DispatcherProvider registry
  Props / ActorSystemConfig
                          // dispatcher selection API

adapter
  TokioDispatcher         // Tokio runtime 向け provider
  PinnedDispatcher        // dedicated lane policy を表す provider
  BlockingDispatcher      // blocking workload policy を表す provider
  EmbassyDispatcher       // 将来の embedded runtime 向け provider
```

`DispatchExecutor`、`DispatchExecutorRunner`、`DispatcherShared`、既存 `DispatcherConfig` は完成状態に残さない。必要な意味論は `Dispatcher` / `DispatcherProvider` / `DispatcherSettings` へ再配置する。

## Decisions

### 1. `Dispatcher` を public trait として導入する

`Dispatcher` は actor mailbox を駆動する runtime object を表す。`DispatcherShared` が現在持つ責務を概念として昇格させる。

少なくとも以下の責務を持つ。

- user/system message の enqueue
- 実行要求の登録
- mailbox pressure / diagnostic publish の反映
- actor ref sender への変換に必要な送信責務

実装表現は `ArcShared<dyn Dispatcher>` を基本とし、shared handle を前提にする。これにより、`DispatcherShared` という concrete wrapper を概念の主語にしない。

### 2. `DispatcherProvider` を dispatcher family の正式境界とする

`DispatcherProvider` は actor ごとに `Dispatcher` を供給する。`PinnedDispatcher` や `TokioDispatcher` はこの trait を実装する policy provider とする。

責務は以下に固定する。

- `DispatcherProvisionRequest` を受け取って actor 用 `Dispatcher` を生成する
- actor 単位に fresh instance を返す必要がある policy (`PinnedDispatcher`) を表現できる
- runtime-specific backend を内部に閉じ込める

`DispatcherProvisionRequest` には最低限以下を含める。

- mailbox
- dispatcher settings
- actor 識別子（thread 名や observability に使える情報）

### 3. `DispatcherConfig` は public abstraction から除去する

既存 `DispatcherConfig` は公開 API の中心から外す。throughput / starvation / schedule adapter のような設定値は `DispatcherSettings` へ移す。

`ActorSystemConfig` と `Props` の public API は以下へ置き換える。

- `with_default_dispatcher_provider(...)`
- `with_dispatcher(id, provider)`
- `with_dispatcher_provider(...)`
- `with_dispatcher_id(...)`

`with_dispatcher_config(...)` と `with_default_dispatcher(DispatcherConfig)` は削除対象とする。

### 4. `Dispatchers` registry は provider registry へ置き換える

現在の `Dispatchers` は `id -> DispatcherConfig` を保持しているが、完成形では `id -> DispatcherProvider` を保持する。

解決フローは以下に統一する。

```text
Props / ActorSystemConfig
  -> dispatcher id or direct provider
  -> Dispatchers registry resolves provider
  -> provider provisions Dispatcher for the actor
```

この方式により、Pekko 的な named dispatcher selection と Rust 的な provider abstraction を両立する。

### 5. `PinnedDispatcher` は dedicated lane policy として再定義する

`PinnedDispatcher` は config factory ではなく、actor ごとに dedicated execution lane を provision する provider とする。

ここでいう dedicated lane は「同一 actor の execution が他 actor と lane を共有しない」ことを意味する。現状の `ThreadedExecutor::execute()` のような「dispatch ごとに thread を spawn する」構造は、Pekko の `PinnedDispatcher` としては不十分であるため採用しない。

この change では `PinnedDispatcher` の意味論を以下に固定する。

- actor ごとに専用 lane を持つ
- lane は actor lifecycle に紐づいて破棄される
- 他 actor と executor queue を共有しない

標準 runtime では dedicated OS thread を基本形とする。Tokio runtime 上で同名 policy を提供するかどうかは別 policy type として扱い、同一名の曖昧な overloading は行わない。

### 6. runtime-specific family は provider 群として adapter に置く

adapter 側は runtime ごとの差分を provider family として提供する。

この change で正規化する family は以下とする。

- `PinnedDispatcher`
- `BlockingDispatcher`
- `TokioDispatcher`

`EmbassyDispatcher` は設計上の拡張先として想定するが、この change では interface compatibility のみ確保し、実装は含めない。

### 7. 互換ブリッジは導入しない

後方互換不要の前提に従い、旧 abstraction を新 abstraction の上に載せる bridge は作らない。

移行方針は単一バッチ置換とする。

- 新しい `Dispatcher` / `DispatcherProvider` / `DispatcherSettings` を導入
- 既存接続点を同一 change 内で付け替え
- 旧 abstraction を同一 change 内で削除

完成状態で legacy 型が残存してはならない。

## Risks / Trade-offs

- 置換範囲は `core` dispatch / actor setup / std adapter にまたがるため、変更面は広い
- `Dispatcher` trait の責務を広く取りすぎると、再び巨大抽象になり得る
- `PinnedDispatcher` の dedicated lane semantics を厳密にやると、runtime resource 管理が増える
- Tokio / embassy を同一 policy 名で雑にまとめると、runtime ごとの差異が隠蔽しきれず設計が壊れる

## Migration Plan

1. `Dispatcher` / `DispatcherProvider` / `DispatcherSettings` / `DispatcherProvisionRequest` を core に導入する
2. `Dispatchers` registry と `ActorSystemConfig` / `Props` の接続点を provider ベースへ置き換える
3. std adapter に `PinnedDispatcher` / `BlockingDispatcher` / `TokioDispatcher` provider 群を追加する
4. actor bootstrap が provider を resolve して actor 用 dispatcher を provision する流れへ置き換える
5. `DispatchExecutor` / `DispatchExecutorRunner` / `DispatcherShared` / `DispatcherConfig` への依存を除去する
6. showcase / cluster / remote / bench など dispatcher 利用箇所を新 API に追随させる
7. 旧 abstraction を削除し、関連 spec / tests / examples を更新する

## Acceptance Shape

完成状態では以下を満たす。

- `Dispatcher` trait が公開 API として存在する
- `DispatcherProvider` が actor/system の dispatcher provisioning 境界として存在する
- `Props` と `ActorSystemConfig` が `DispatcherConfig` を public には受け取らない
- `PinnedDispatcher` が dedicated lane policy として provider family に含まれる
- std adapter は core の `ActorSystem` や façade wrapper を再公開しない
- `DispatchExecutor`、`DispatchExecutorRunner`、`DispatcherShared`、既存 `DispatcherConfig` は完成状態に残らない
