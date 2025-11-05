# Typed Actor Layer Design (WIP)

## 1. ゴールと前提
- 現行の `ActorRef`/`ActorContext`/`ActorSystem` はそのまま維持し、Typed レイヤーはラッパーとして提供する。
- Pekko Typed および protoactor-go の設計を参照しつつ、Rust のジェネリクスで強制される型境界を採用する。
- OpenSpec で定義した要件（TypedActorSystem Generic Boundary、Typed Spawn API、MessageAdapter 等）を実現するための API 面と内部構造を整理する。
- 参照資料: `references/pekko/docs/src/main/paradox/typed/interaction-patterns.md`（Message Adapter の制約や運用方法）、`references/pekko/actor-typed/src/main/scala/.../ActorContext.scala`（Adapter 登録 API）。

## 2. レイヤー構成
1. **Untyped Runtime Layer**: 既存の Props, Mailbox, Scheduler, ActorSystem。
2. **Typed Kernel**: `TypedActorSystem<M>`, `TypedActorRef<M>`, `TypedActorContext<M>` が配置され、Untyped を内部的に利用。
3. **Adapters & Tooling**: MessageAdapter, BehaviorAdapter, ask パターン補助、テストキット。

Typed Kernel は Untyped Layer の API を少数（spawn, stop, send, watch 等）に限定して利用し、その他の詳細（mailbox 種別など）は builder が吸収する。

## 3. TypedActorSystem<M>
- `TypedActorSystem<M>` は `Arc<TypedActorSystemInner>` を保持し、Inner には既存の `ActorSystem` 参照と `PhantomData<M>` を持たせる。
- API:
  - `fn builder(config: RuntimeConfig) -> TypedActorSystemBuilder<M>`
  - `fn root_behavior(&self) -> Behavior<M>`（もしくは builder 経由で設定）
  - `fn spawn_root(&self, behavior: Behavior<M>, opts: SpawnOpts) -> TypedActorRef<M>`
- `spawn_root` 実装では Behavior を `Props` に変換し untyped `spawn` を呼ぶが、変換は Typed 側の DSL が担当。
- サブツリー用に `TypedActorContext<M>::spawn<N>(&self, behavior: Behavior<N>, opts)` を定義し、`N` と `M` の整合チェックは Context 側で実施（`N: Into<M>` などの境界を検討）。

## 4. Behavior モデル
- `Behavior<M>` は enum で `Receive(fn(&mut State, &TypedActorContext<M>, M) -> Behavior<M>)`, `Same`, `Stopped`, `Unhandled` を持つ設計を想定。
- DSL として `Behaviors::setup`, `Behaviors::receive_message`, `Behaviors::receive_signal`, `Behaviors::same`, `Behaviors::stopped` を提供し、Pekko の `Behaviors.setup/receive` と同じ書き味を目指す。
- `Behaviors::setup` 内では初期状態生成と MessageAdapter 登録を行い、`receive_message`/`receive_signal` でそれぞれメッセージとシグナル（`PostStop`, `PreRestart` 等）を処理し、クロージャは必ず次の `Behavior<M>` を返して状態遷移を明示する（例: `Behaviors::receive_message(|ctx, msg| NextBehavior::from(msg))`).
- `Behaviors::same` は副作用だけ実行して状態遷移を変えない場合、`Behaviors::stopped` は明示停止シナリオのために使用し、どちらも enum バリアントに直接マップ。
- ランレベルでは Behavior を untyped mailbox が実行できるよう trait `TypedBehaviorDriver` を導入し、メッセージ受信時に `TypedActorContext` を生成してコールバック。

## 5. Typed Spawn API と Props 移行
- ドキュメントでは「untyped の `spawn(props: Props, …)` に対して、typed では `spawn(behavior: Behavior<M>, opts: SpawnOpts)` を利用する」ことを明示。
- `SpawnOpts` は `MailboxConfig`, `DispatcherConfig`, `SupervisorStrategy`, `RestartPolicy` 等の Props 情報をそのまま受け取り、内側で既存 `Props` に変換してから untyped runtime へ渡す。
- `SpawnOpts::mailbox(cfg)` のような builder スタイルにし、`Behavior` 設定と組み合わせて従来の柔軟性（bounded/unbounded mailbox、優先度付けなど）を失わないようにする。
- これにより利用者は Behavior を直接記述するだけでよくなり、MessageAdapter や監視戦略は Behavior DSL 内に閉じ込められる。

## 6. MessageAdapter の設計
- `TypedActorContext<M>` に以下の API を追加:
  ```rust
  pub fn message_adapter<S, F>(&self, f: F) -> TypedActorRef<S>
  where
      F: Fn(S) -> M + Send + Sync + 'static,
  ```
- Adapter は内部で `ActorRef<S>` を生成し、受信した `S` をクロージャで `M` にマップして元アクターへ `TypedActorRef<M>` として配送する。
- Pekko での制約を踏襲し、同一 `S` 型につき登録できる Adapter は 1 つに制限（`HashMap<TypeId, AdapterHandle>` で管理）。
- Adapter のライフサイクルは親アクターに従い、Context drop 時に自動解除。Pekko の注意点（例: Adapter が panic するとアクター停止）も踏襲し、クロージャ内パニックを監視戦略で扱う。

## 7. Cross-Protocol 通信フロー
1. `TypedActorRef<OrderCommand>` が `InventoryEvent` を受信したい場合、Context で `message_adapter::<InventoryEvent, OrderCommand>` を登録。
2. Adapter は `TypedActorRef<InventoryEvent>` を返し、これを Inventory アクターへ共有。
3. Inventory イベント受信時に変換クロージャが走り、`OrderCommand::InventoryUpdated` 等に変換されて元アクターへキューイング。
4. Adapter 未登録で異なる型を送った場合はコンパイルエラー、または `AdapterNotFound` エラーを返す。

## 8. Supervisor とライフサイクル
- `TypedSupervisor` DSL を提供し、`Behavior` を `supervise().on_failure(strategy)` でラップ。
- 実装上は Behavior ツリーに `SupervisorDecorator` を挿入し、Untyped の再起動 API にブリッジ。
- 監視戦略の設定は SpawnOpts でも Behavior DSL でも指定できるが、優先順序をドキュメント化（例: Behavior 側 > SpawnOpts > デフォルト）。

## 9. async fn アダプタ
- `BehaviorAdapter::from_future(ctx, future, map_ok, map_err)` のような API を提供し、Future 完了時に MessageAdapter と同様に `M` へ変換して `ctx.self_ref()` へ送信。
- 将来的に `async fn` で Behavior を書きたい場合に備え、`receive_async` の試験的 API も検討。ただし初期段階では Hook のみ導入。

## 10. ドキュメント計画
- README / guides に「untyped spawn(props) から typed spawn(behavior) への移行」「MessageAdapter 必須ケース」を記載。
- 例示として `OrderCommand` / `InventoryEvent` を利用し、Type mismatch → Adapter 登録 → 正常動作のフローを説明。
