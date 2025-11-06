## ADDED Requirements

### Requirement: Typed Actor System Bootstrapping
Typed Actor System MUST expose a root guardian Behavior and builder API that wires mailbox/runtime初期化を隠蔽しつつ、型付きメッセージ境界を維持すること。

#### Scenario: ルートガーディアン経由でスパウン
- **GIVEN** Runtime 設定と `Behavior<RootCommand>` が提供されている
- **WHEN** 利用者が Typed Actor System Builder で `spawn_root` を呼び出す
- **THEN** ルートガーディアン下にアクターが生成され、untyped メールボックス設定は内部に封じ込められる
- **AND** ルートガーディアンが停止するとシステム全体が orderly shutdown する

### Requirement: TypedActorSystem Generic Boundary
`TypedActorSystem<M>` MUST treat the type parameter `M` as the sole message type that the system-level guardian can receive, ensuring that spawn APIs enforce compatibility against `M` またはそのサブ型に限定されなければならない。

#### Scenario: 不整合メッセージ型のスパウン拒否
- **GIVEN** `TypedActorSystem<OrderCommand>` と `Behavior<InventoryCommand>` がある
- **WHEN** 利用者がこのシステムで `Behavior<InventoryCommand>` を `spawn_root` しようとする
- **THEN** 型不一致としてビルドエラーが発生し、`OrderCommand` 互換 Behavior 以外は受け入れられない

### Requirement: Typed Spawn API
Typed レイヤー MUST offer `spawn(behavior: Behavior<M>, opts: SpawnOpts)` など Behavior を直接受け取る API を提供し、untyped の `spawn(props: Props, …)` を置き換える形で Document 化されなければならない。また `SpawnOpts` MUST expose mailbox 設定（`MailboxConfig` 等）や dispatcher/supervisor など Props 相当の構成を受け渡せなければならない。

#### Scenario: Props から Behavior への移行が明示
- **GIVEN** 既存ドキュメントが untyped では `spawn(props: Props)` と説明している
- **WHEN** Typed レイヤーのガイドを参照する
- **THEN** `spawn(behavior: Behavior<OrderCommand>)` の記述があり、Props ではなく Behavior を渡す手順が明確に案内される

#### Scenario: Mailbox 設定を引き継げる
- **GIVEN** untyped で `Props.with_mailbox(MailboxConfig::bounded(1024))` を利用している
- **WHEN** Typed API で `spawn(behavior, SpawnOpts::default().mailbox(MailboxConfig::bounded(1024)))` を指定する
- **THEN** 指定した MailboxConfig が Untyped runtime に伝搬し、既存と同じ挙動でメッセージ配送される

### Requirement: Typed Actor References
TypedActorRef MUST restrict sending to a single message type `M`, and Envelope や Broadcast など内部メッセージは TypedActorContext 経由で抽象化されなければならない。

#### Scenario: TypedActorRef 型違いの拒否
- **GIVEN** `TypedActorRef<OrderCommand>` と `TypedActorRef<InventoryCommand>` が存在する
- **WHEN** 利用者が `InventoryCommand` を `TypedActorRef<OrderCommand>` に送ろうとする
- **THEN** コンパイルエラーまたは明示的なトランスレータが無い限りビルドに失敗する

### Requirement: Message Adapters For Incoming Protocol Alignment
Typed レイヤー MUST provide `MessageAdapter` を通じて「自アクターが受信したい異なるプロトコル」を自プロトコルに合わせる手段を提供しなければならない。送信側は相手の `TypedActorRef<TheirCommand>` に直接メッセージを送信できる自由度を維持し、受信側が必要に応じて Adapter で変換・ラップする設計を必須とする。

#### Scenario: 異種イベント受信を自己プロトコルに写像
- **GIVEN** `TypedActorRef<OrderCommand>` が `InventoryEvent` を発行するアクターからイベントを受信したい
- **WHEN** `TypedActorContext` で `message_adapter::<InventoryEvent, OrderCommand>(...)` を登録する
- **THEN** 受信した `InventoryEvent` が `OrderCommand::InventoryUpdated` などへ変換され、`OrderCommand` プロトコルのまま処理できる一方、送信側は Adapter なしで `TypedActorRef<InventoryEvent>` に向けて送信できる

### Requirement: Behavior Lifecycle Contract
Behavior MUST be modeled as a pure function that takes `TypedActorContext<M>` and returns `Behavior<M>`, かつ `Receive`, `Stopped`, `Same` などの明示的な戻り値で状態遷移を表現しなければならない。

#### Scenario: 状態遷移の明示
- **GIVEN** カウンターアクターの Behavior が `count` 状態を保持している
- **WHEN** `Increment` を処理して新しい `count` を返す
- **THEN** Behavior は `Receive::new_state(next_count)` を返し、副作用は TypedActorContext API に閉じ込められる

### Requirement: Behavior DSL Coverage
Typed レイヤー MUST provide DSL helpers such as `Behaviors::setup`, `Behaviors::receive_message`, `Behaviors::receive_signal`, `Behaviors::same`, `Behaviors::stopped` so that利用者は Pekko Typed 相当のパターンで Behavior を構築できなければならず、`receive_message`/`receive_signal` のクロージャは常に次の `Behavior<M>` を返して状態遷移を明示できなければならない。

#### Scenario: DSL を用いた初期化とシグナル処理
- **GIVEN** 新規アクターを `Behaviors::setup(|ctx| { ... })` で初期化し、同一 Behavior 内で `Behaviors::receive_message` と `Behaviors::receive_signal` を組み合わせたい
- **WHEN** Typed レイヤーの DSL を参照する
- **THEN** `receive_message(|ctx, msg| { state.update(msg); Behaviors::same() })` のようにクロージャが次の `Behavior` を返し、状態更新／停止処理が DSL だけで完結する

### Requirement: Supervision Strategies
Typed 層 MUST define OneForOne/AllForOne および Backoff 付き戦略を備え、Behavior 定義と組み合わせて宣言的に適用できる DSL を提供しなければならない。

#### Scenario: 宣言的スーパーバイザ適用
- **GIVEN** 子アクター Behavior と `Supervisor::one_for_one().with_backoff(min, max)` の設定がある
- **WHEN** 親が `spawn_with_supervisor` を呼び出す
- **THEN** 子の失敗は設定済み戦略で再起動/停止が決定され、親コード側で個別例外処理を書く必要がない

### Requirement: Async Adapter Hooks
将来の async fn ベース Behavior を受け入れるため、Typed レイヤー MUST expose `BehaviorAdapter`/`MessageAdapter` などの Hook を公開し、非同期タスク完了を型付きメッセージへ安全にブリッジできなければならない。

#### Scenario: TypedActorRef への安全な非同期橋渡し
- **GIVEN** 長時間処理を行う async タスクと `TypedActorRef<Response>` がある
- **WHEN** タスク完了 Future が `BehaviorAdapter::from_future` に渡される
- **THEN** 完了結果が元のメッセージ型に変換され、Typed Actor は未定義挙動なく受信できる

### Requirement: Compatibility With Existing Untyped Runtime
TypedActorRef/TypedActorContext/TypedActorSystem MUST be constructed asラッパーまたは拡張であり、既存の ActorRef, ActorContext, ActorSystem を改変せずに動作しなければならない。

#### Scenario: 既存ランタイムを変更せず導入
- **GIVEN** 現行の ActorRef/ActorContext/ActorSystem 実装が存在する
- **WHEN** Typed レイヤーが導入される
- **THEN** 既存実装のシグネチャや振る舞いを変更する必要はなく、Typed コンポーネントは上位レイヤーとしてこれらを安全にラップする
