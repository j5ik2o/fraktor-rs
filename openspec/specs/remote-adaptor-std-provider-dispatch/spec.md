# remote-adaptor-std-provider-dispatch Specification

## Purpose
TBD - created by archiving change remote-redesign. Update Purpose after archive.
## Requirements
### Requirement: StdRemoteActorRefProvider 型

`fraktor_remote_adaptor_std_rs::provider::StdRemoteActorRefProvider` 型が定義され、**loopback 振り分けを含む ActorPath 解決の唯一の窓口** として機能する SHALL。この型が core spec `remote-core-actor-ref-provider` の「loopback 短絡の実装責務は adapter にある」要件 (Decision 3-C) を満たす実装である。

#### Scenario: 型の存在

- **WHEN** `modules/remote-adaptor-std/src/provider/dispatch.rs` を読む
- **THEN** `pub struct StdRemoteActorRefProvider` が定義されている

#### Scenario: フィールド構成

- **WHEN** `StdRemoteActorRefProvider` のフィールドを検査する
- **THEN** 以下を含む:
  - `local_address: UniqueAddress` (ローカルノードの UniqueAddress)
  - `local_provider: ActorRefProviderShared<LocalActorRefProvider>` または同等の actor-core 公開型 (actor-core の local actor ref provider 参照)
  - `remote_provider: Box<dyn RemoteActorRefProvider>` (core の remote provider 参照)

### Requirement: adapter 側エラー型

`StdRemoteActorRefProvider` は core の `ProviderError` をそのまま外へ漏らさず、adapter 側の分岐・委譲・sender 構築失敗まで表現できる専用エラー型 `StdRemoteActorRefProviderError` (名称は同等なら可) を持つ SHALL。

#### Scenario: 型の存在

- **WHEN** `modules/remote-adaptor-std/src/provider/` 配下を検査する
- **THEN** `pub enum StdRemoteActorRefProviderError` または同等の adapter 専用エラー型が定義されている

#### Scenario: core ProviderError のラップ

- **WHEN** remote 分岐で `self.remote_provider.actor_ref(path)` または `watch/unwatch` が `ProviderError` を返す
- **THEN** adapter 側では `StdRemoteActorRefProviderError::CoreProvider(ProviderError)` または同等の形で保持される

#### Scenario: local provider エラーのラップ

- **WHEN** local 分岐で `self.local_provider.actor_ref(local_equivalent_path)` が `ActorError` を返す
- **THEN** adapter 側では `StdRemoteActorRefProviderError::LocalProvider(ActorError)` または同等の形で保持される

### Requirement: actor_ref メソッドの振り分けロジック

`StdRemoteActorRefProvider::actor_ref` は `ActorPath` 解決の統合窓口として、以下の 3 分岐で振る舞う SHALL。戻り値のエラー型は `ProviderError` ではなく、adapter 専用の `StdRemoteActorRefProviderError` とする。

1. **authority なし path**: 通常の local path とみなし、そのまま `local_provider` に委譲する  
2. **authority ありで local ノードを指す path**: loopback/canonical local path とみなし、**authority を持たない等価な local path に正規化してから** `local_provider` に委譲する  
3. **authority ありで `self.local_address` と不一致**: remote path とみなし、`remote_provider` を呼ぶ  

ここでいう「local ノードを指す」は、**Address (scheme/system/host/port) が `self.local_address.address` と一致し、uid は path 側が non-zero の場合のみ比較対象にする** ことを意味する。path 側 uid が `0` の場合は「uid 未確定」とみなし wildcard として扱う。これは actor-core の canonical path が uid を常に含むとは限らず、design.md Decision 13 でも `uid=0` を sentinel として許容しているためである。

これは actor-core の `LocalActorRefProvider` が authority 付き path を受け付けないためであり、かつ Pekko 互換の観点で authority なし local path も同じ provider 窓口で解決できるようにするためである。

#### Scenario: authority なし local path の振り分け

- **WHEN** `path.parts().authority_endpoint().is_none()` の状態で `StdRemoteActorRefProvider::actor_ref(path)` を呼ぶ
- **THEN** `resolve_remote_address(&path)` の結果に依存せず、内部で `self.local_provider.actor_ref(path)` が呼ばれ、その戻り値 (local ActorRef) が返される

#### Scenario: authority あり local path の振り分け

- **WHEN** `path` の Address 部分 (scheme/system/host/port) が `self.local_address.address` と一致し、かつ path 側 uid が `0` または `self.local_address.uid` と一致する状態で `StdRemoteActorRefProvider::actor_ref(path)` を呼ぶ
- **THEN** adapter は `path` を authority なしの local 等価 path に正規化し、その正規化後 path に対して `self.local_provider.actor_ref(local_equivalent_path)` を呼ぶ。その戻り値 (local ActorRef) が返される

#### Scenario: authority を剥がした local 等価 path への正規化

- **WHEN** authority 付きだが local ノードを指す path を local 経路へ振り分ける
- **THEN** system 名・guardian・path segments・uid を保ったまま authority だけを除去した `ActorPath` が構築される。`LocalActorRefProvider` に authority 付き path をそのまま渡してはならない

#### Scenario: uid=0 は wildcard として扱う

- **WHEN** `resolve_remote_address(&path)` の結果の uid が `0` で、Address 部分は `self.local_address.address` と一致している
- **THEN** uid 不一致だけを理由に remote 分岐へ送ってはならず、local 分岐として扱う

#### Scenario: uid が両方 non-zero の場合は厳密比較

- **WHEN** `resolve_remote_address(&path)` の結果の uid が non-zero で、`self.local_address.uid` も non-zero かつ両者が異なる
- **THEN** その path は local 分岐ではなく remote 分岐として扱われる

#### Scenario: リモート path の振り分け

- **WHEN** `path` の authority が `self.local_address` と一致しない状態で `StdRemoteActorRefProvider::actor_ref(path)` を呼ぶ
- **THEN** 内部で以下が順に実行される:
  1. `self.remote_provider.actor_ref(path)` を呼んで `RemoteActorRef` を取得
  2. `RemoteActorRef` を元に remote sender (内部で `TcpRemoteTransport` 経由の送信を行う) を構築
  3. remote sender を actor-core の `ActorRef` にラップして返す

#### Scenario: sender 構築失敗の表現

- **WHEN** `RemoteActorRef` から remote sender を構築する過程で adapter 側 wiring 失敗が起きる
- **THEN** 戻り値は `StdRemoteActorRefProviderError::RemoteSenderBuildFailed` または同等の adapter 専用エラーになる

#### Scenario: core provider への local path 混入の防止

- **WHEN** ローカル path に対して `StdRemoteActorRefProvider::actor_ref(path)` が呼ばれる
- **THEN** `self.remote_provider.actor_ref(path)` は **呼ばれない** (振り分けは adapter 側で完結し、core provider は remote path のみを受け取る)

### Requirement: actor_ref メソッドの remote 振り分け

`StdRemoteActorRefProvider::actor_ref` は actor-core provider surface の remote 分岐で、送信時に std remote event loop へ到達する `ActorRef` を返さなければならない（MUST）。

#### Scenario: remote-aware provider は ActorSystemConfig 経由で登録される

- **GIVEN** std remote adapter が remote-aware actor-ref provider installer または同等の builder helper を提供している
- **WHEN** caller がそれを `ActorSystemConfig::with_actor_ref_provider_installer` に渡して `ActorSystem::create_with_config` を呼ぶ
- **THEN** actor system は `StdRemoteActorRefProvider` 相当の provider を actor-core provider surface に登録する
- **AND** caller は `StdRemoteActorRefProvider::actor_ref` を直接呼ばなくても `ActorSystem::resolve_actor_ref(remote path)` を使える

#### Scenario: リモート path の振り分けは配送経路に接続される

- **WHEN** `path` の authority が local address と一致しない状態で `StdRemoteActorRefProvider::actor_ref(path)` を呼ぶ
- **THEN** provider は remote path 用の `ActorRef` を返す
- **AND** その sender は `RemoteActorRefSender` 相当であり、`ActorRefSender::send` 呼び出し時に `RemoteEvent::OutboundEnqueued` を adapter 内部 sender に push する
- **AND** その event は `RemoteShared` event loop に処理され、対象 peer が接続済みかつ payload がサポート対象なら `TcpRemoteTransport::send` まで到達する

#### Scenario: provider dispatch から transport send までを contract test で確認する

- **WHEN** external integration test が actor-core provider surface から remote path を resolve し、サポート対象 payload を tell する
- **THEN** test は `RemoteEvent::OutboundEnqueued` の push だけでなく、`TcpRemoteTransport::send` が envelope frame を writer に enqueue したことまで観測する
- **AND** sender channel full / closed は caller が観測できる `SendError` に変換される

#### Scenario: ActorSystem::resolve_actor_ref から remote sender へ到達する

- **GIVEN** `ActorSystemConfig::with_actor_ref_provider_installer` で remote-aware provider が登録された actor system がある
- **WHEN** caller が `ActorSystem::resolve_actor_ref(remote path)` を呼び、返された `ActorRef` へサポート対象 payload を tell する
- **THEN** call path は `StdRemoteActorRefProvider` 相当の provider を通る
- **AND** `RemoteActorRefSender` は `RemoteEvent::OutboundEnqueued` を adapter 内部 sender に push する
- **AND** test は provider を直接 new して呼ぶだけで済ませない

### Requirement: path 解決の委譲

`StdRemoteActorRefProvider` は **authority あり path** から `UniqueAddress` を抽出する際、core の `resolve_remote_address` 関数を利用する SHALL。authority なし path は local path として先に処理されるため、この関数による比較を必須とはしない。比較時は `UniqueAddress` の完全一致ではなく、Address 一致 + uid wildcard 規則を使う。

#### Scenario: authority あり path での resolve_remote_address の利用

- **WHEN** authority あり path を扱う `StdRemoteActorRefProvider::actor_ref` の実装を検査する
- **THEN** `fraktor_remote_core_rs::domain::provider::resolve_remote_address(&path)` が呼ばれ、結果が `Some(UniqueAddress)` で `self.local_address` と比較される

#### Scenario: local path 判定後の入力条件適合

- **WHEN** `resolve_remote_address(&path)` の結果が local ノードを指すと判定されたため local 経路に振り分ける
- **THEN** `LocalActorRefProvider` の入力条件 (authority を持たない path のみ受理) を満たすよう、adapter 側で authority 除去済み path に変換してから委譲する

### Requirement: watch / unwatch の remote forwarding

`StdRemoteActorRefProvider` は `watch` / `unwatch` メソッドを持つ場合、それらは **remote path 専用** の forwarding helper として振る舞う SHALL。リモート path に対しては core の remote provider の `watch` / `unwatch` に委譲し、ローカル path に対しては `Err(StdRemoteActorRefProviderError::NotRemote)` を返す。ローカル death watch は `actor_ref(path)` が返した local `ActorRef` に対して actor-core の通常経路 (`ActorContext::watch` 等) で扱い、この型が local watch の窓口を兼ねてはならない。

#### Scenario: リモート path の watch

- **WHEN** リモート path に対して `StdRemoteActorRefProvider::watch(watchee, watcher)` を呼ぶ
- **THEN** 内部で `self.remote_provider.watch(watchee, watcher)` が呼ばれる

#### Scenario: ローカル path の watch は拒否

- **WHEN** ローカル path に対して `StdRemoteActorRefProvider::watch(watchee, watcher)` を呼ぶ
- **THEN** `Err(StdRemoteActorRefProviderError::NotRemote)` が返り、`self.remote_provider.watch` は呼ばれない

#### Scenario: ローカル death watch は actor-core 通常経路

- **WHEN** ローカル actor を watch するユースケースを設計書として確認する
- **THEN** `StdRemoteActorRefProvider::watch` ではなく、解決済み local `ActorRef` に対する actor-core の通常経路 (`ActorContext::watch` 等) を使うことが明記されている
