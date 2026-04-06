## ADDED Requirements

### Requirement: 単一の RemoteActorRefProvider trait (remote 専用)

`fraktor_remote_core_rs::provider::RemoteActorRefProvider` trait が定義され、**リモート経路の actor ref 生成・死活管理の窓口** となる SHALL。Pekko `RemoteActorRefProvider` (Scala, 784行) に対応するが、fraktor-rs では責務を分離する:

- **core の `RemoteActorRefProvider`**: リモート ActorPath に対する `RemoteActorRef` の構築と watch 管理のみを担う。local ActorPath の解決には関与しない
- **loopback (同一 `UniqueAddress`) のディスパッチ**: Phase B adapter の責務。adapter が `ActorPath` を検査して local / remote を振り分け、local なら actor-core の local provider に委譲、remote なら core の `RemoteActorRefProvider` を呼ぶ
- **理由**: core は `fraktor-actor-core-rs` の local provider を保持しないため、core 内で local ActorRef を構築する手段がない。loopback 短絡を core の Requirement にすると、Phase A では実装不能な責務の先食いになる

#### Scenario: trait の存在

- **WHEN** `modules/remote-core/src/provider/` を検査する
- **THEN** `pub trait RemoteActorRefProvider` が定義されている

#### Scenario: 単一の provider trait

- **WHEN** `modules/remote-core/src/provider/` 配下を検査する
- **THEN** `RemoteActorRefProvider` trait が1つのみ存在し、`LoopbackRemoteActorRefProvider`・`TokioRemoteActorRefProvider` 等の名前空間が異なる variant trait は存在しない

#### Scenario: provider 兄弟ファイルの不在

- **WHEN** `modules/remote-core/src/provider/` 配下を検査する
- **THEN** `loopback.rs`・`tokio.rs`・`remote.rs` のような「transport 種別ごと provider」を示すファイル名が存在しない

### Requirement: actor_ref メソッド (remote path 専用)

`RemoteActorRefProvider` trait は `actor_ref` メソッドを持ち、**リモート `ActorPath`** (local ではない authority を持つ path) から `RemoteActorRef` を生成する SHALL。呼び出し元は事前にローカル path と remote path を区別する責務を負う (Phase B adapter がこの振り分けを行う)。`actor_ref` に local path を渡した場合の挙動は Error または未定義として扱う (Scenario で明確化)。

`actor_ref` は **`&mut self`** を取る。これは純粋な query ではなく、内部で以下のような状態更新を伴う実装を許容するためである (CQS 例外、Decision として明記):
- 初回解決時にリモートアドレス → `RemoteNodeId` のマッピングを内部キャッシュへ登録
- 新しい remote authority に対する watch entry の遅延初期化
- Pekko `RemoteActorRefProvider.actorFor` の remote 部分と同等の caching 動作

#### Scenario: actor_ref メソッドのシグネチャ

- **WHEN** `RemoteActorRefProvider::actor_ref` の定義を読む
- **THEN** `fn actor_ref(&mut self, path: ActorPath) -> Result<RemoteActorRef, ProviderError>` または同等のシグネチャが宣言されている

#### Scenario: 戻り値は RemoteActorRef である

- **WHEN** `RemoteActorRefProvider::actor_ref` の戻り値型を検査する
- **THEN** 戻り値は `fraktor_remote_core_rs::provider::RemoteActorRef` である (actor-core の polymorphic `ActorRef` は返さない)

#### Scenario: local path に対するエラー返却

- **WHEN** `path` が local authority (Phase B で注入された `UniqueAddress` と一致する authority) を持つ状態で `actor_ref(path)` を呼ぶ
- **THEN** `Err(ProviderError::NotRemote)` または同等のエラーが返る。または、呼び出し前に adapter が振り分ける契約のもとで、この Scenario は「入力契約違反」として Error 扱いする (adapter が local を振り分けてから本メソッドを呼ぶことを前提)

#### Scenario: &mut self の rustdoc 記述

- **WHEN** `actor_ref` の rustdoc を読む
- **THEN** `&mut self` を取る理由 (キャッシュ動作、CQS 例外) および「local path は事前に adapter が振り分ける前提」が明記されている

#### Scenario: ProviderError 型の存在

- **WHEN** `modules/remote-core/src/provider/` 配下を検査する
- **THEN** `pub enum ProviderError` が定義され、`NotRemote`・`InvalidPath`・`MissingAuthority`・`UnsupportedScheme` 等のバリアントを含む

### Requirement: watch / unwatch メソッド

`RemoteActorRefProvider` trait は `watch` と `unwatch` メソッドを持ち、リモートアクターに対する death watch の登録/解除を受け付ける SHALL。これは Pekko `RemoteActorRefProvider` が `RemoteWatcher` actor と協調してリモート死活監視を行う責務に対応する。actual な heartbeat 送受信とタイマー駆動は Phase B で adapter 側の `watcher_actor/` が担うが、trait レベルの宣言と path → remote node 解決は core の provider の責務。

#### Scenario: watch メソッドのシグネチャ

- **WHEN** `RemoteActorRefProvider::watch` の定義を読む
- **THEN** `fn watch(&mut self, watchee: ActorPath, watcher: Pid) -> Result<(), ProviderError>` または同等のシグネチャが宣言されている

#### Scenario: unwatch メソッドのシグネチャ

- **WHEN** `RemoteActorRefProvider::unwatch` の定義を読む
- **THEN** `fn unwatch(&mut self, watchee: ActorPath, watcher: Pid) -> Result<(), ProviderError>` または同等のシグネチャが宣言されている

#### Scenario: &mut self の維持

- **WHEN** `watch` および `unwatch` の self 引数を検査する
- **THEN** 両方とも `&mut self` を取る (内部 watch entries の変更を伴うため)

#### Scenario: 内部可変性を使わない

- **WHEN** `watch` および `unwatch` の実装を検査する
- **THEN** `SpinSyncMutex::lock()`・`RefCell::borrow_mut()` 等の内部可変性プリミティブを使わない (これらは adapter 側で wrap する)

### Requirement: RemoteActorRef data 型

`fraktor_remote_core_rs::provider::RemoteActorRef` 型が定義され、リモートアクター参照を表現する **data-only 型** である SHALL。これは `RemoteActorRefProvider::actor_ref` メソッドの公開戻り値であり、Phase B で adapter 側が `RemoteActorRef` を受け取って actor-core の `ActorRef` + remote 用 `ActorRefSender` を構築する。core の `RemoteActorRef` 自体は path と remote_node の保持、accessor、clone、equality のみを提供し、送信責務は持たない。

#### Scenario: RemoteActorRef の存在

- **WHEN** `modules/remote-core/src/provider/remote_actor_ref.rs` を読む
- **THEN** `pub struct RemoteActorRef` が定義され、`ActorPath` と `RemoteNodeId` (または `UniqueAddress`) を保持する

#### Scenario: 送信メソッドを持たない

- **WHEN** `RemoteActorRef` の公開メソッド一覧を検査する
- **THEN** `send`・`tell`・`ask` 等のメッセージ送信メソッドは定義されていない (送信は Phase B で adapter 側の `Association::enqueue` が担う)

#### Scenario: accessor のみ公開

- **WHEN** `RemoteActorRef` の impl ブロックを検査する
- **THEN** 公開メソッドは `path(&self) -> &ActorPath`、`remote_node(&self) -> &RemoteNodeId`、`Clone`・`PartialEq`・`Eq`・`Hash` 実装等の純粋 accessor / trait impl のみである

### Requirement: loopback transport の不要化 (概念レベル)

`remote-core` は loopback (同一 `UniqueAddress` 内の actor 間配送) 用の transport を提供しない SHALL。loopback は「transport の一形態」ではなく「adapter の path 解決段階での短絡」として扱う。これにより、Pekko の挙動 (同一 system 内は `LocalActorRefProvider` が直接解決) と整合し、loopback 専用の transport 実装を core に置く必要がなくなる。

**注意**: loopback 短絡の **実装責務は Phase B adapter** にある。core の `RemoteActorRefProvider::actor_ref` は remote path のみを受け付け、adapter がその前段で `ActorPath` の authority を検査して local / remote に振り分ける。

#### Scenario: LoopbackTransport 型の不在

- **WHEN** `modules/remote-core/src/transport/` 配下を検査する
- **THEN** `LoopbackTransport` 型が存在しない。もしくは、存在する場合は `#[cfg(any(test, feature = "test-support"))]` で gate された「テスト用 fake transport」として明示的にマークされ、production 経路では `RemoteTransport` trait の実装として provider が自動選択しない

#### Scenario: loopback 短絡の adapter 責務明示

- **WHEN** `remote-core` の `RemoteActorRefProvider` trait と関連 doc comment を読む
- **THEN** 「local path の振り分けは adapter 責務であり、core provider は remote path のみを扱う」旨が明記されている

#### Scenario: core は local provider を持たない

- **WHEN** `remote-core/src/provider/` 配下を検査する
- **THEN** local ActorRef を構築する型 (例: `LocalActorRef`、`LoopbackActorRefProvider`) は存在しない。core 側に local dispatch の実装は含まれない

### Requirement: ActorPath → UniqueAddress 解決関数

`fraktor_remote_core_rs::provider::resolve_remote_address` 関数が定義され、`ActorPath` から `UniqueAddress` (host, port, system, uid) を抽出する SHALL。状態を持たないため struct ではなく free function として実装する。

#### Scenario: 関数の存在

- **WHEN** `modules/remote-core/src/provider/path_resolver.rs` を読む
- **THEN** `pub fn resolve_remote_address(path: &ActorPath) -> Option<UniqueAddress>` が宣言されている

#### Scenario: struct PathResolver の不在

- **WHEN** `modules/remote-core/src/provider/` 配下を検査する
- **THEN** `pub struct PathResolver` は存在しない (状態を持たない純関数であるため)

### Requirement: provider 状態の &mut self 操作

`RemoteActorRefProvider` trait の状態変更メソッド (`actor_ref`, `watch`, `unwatch` 等) はすべて `&mut self` を取り、内部可変性を使わない SHALL。各メソッドの具体的なシグネチャ要件はそれぞれの Requirement で既に定義済み (`actor_ref メソッド`、`watch / unwatch メソッド`)。この Requirement は、将来 trait に新たな状態変更メソッドを追加する際のメタ規約として機能する。

#### Scenario: trait の全状態変更メソッドが &mut self

- **WHEN** `RemoteActorRefProvider` trait の全メソッドを列挙する
- **THEN** 状態変更を伴うメソッド (`actor_ref`, `watch`, `unwatch`, 将来追加されるもの) はすべて `&mut self` を取る

#### Scenario: 内部可変性プリミティブの不在

- **WHEN** `RemoteActorRefProvider` の実装コード (core 側、Phase A では trait のみ) を検査する
- **THEN** `SpinSyncMutex`・`Cell`・`RefCell`・`AShared` 等の内部可変性プリミティブを trait メソッドの実装内で使わない
