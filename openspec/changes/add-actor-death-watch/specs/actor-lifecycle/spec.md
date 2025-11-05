## ADDED Requirements

### Requirement: SystemMessageにWatch/Unwatch/Terminatedを追加する
`SystemMessage`列挙型に`Watch(Pid)`、`Unwatch(Pid)`、`Terminated(Pid)`variantを追加する（MUST）。

#### Rationale
- Akka/PekkoのDeathWatchと同じシステムメッセージ方式を採用
- 監視者のメールボックスに直接配送されるため、順序保証と効率性を両立
- EventStreamを経由しないため、O(n)の効率（n=監視者数）

#### Scenario: SystemMessage::Watchで監視を開始する
- **GIVEN** `ActorContext::watch(target)`が呼ばれた場合
- **WHEN** `SystemMessage::Watch(watcher_pid)`が対象アクターに送信される
- **THEN** 対象アクターのwatchersリストに監視者のPidが追加される
- **AND** 同じPidを複数回追加しても冪等（重複しない）

#### Scenario: SystemMessage::Unwatchで監視を解除する
- **GIVEN** `ActorContext::unwatch(target)`が呼ばれた場合
- **WHEN** `SystemMessage::Unwatch(watcher_pid)`が対象アクターに送信される
- **THEN** 対象アクターのwatchersリストから監視者のPidが削除される
- **AND** リストにいないPidを削除しても無害（エラーにならない）

#### Scenario: SystemMessage::Terminatedで停止を通知する
- **GIVEN** 監視対象アクターが停止した場合
- **WHEN** `ActorCell::notify_watchers_on_stop()`が呼ばれる
- **THEN** 各監視者のメールボックスに`SystemMessage::Terminated(stopped_pid)`が送信される
- **AND** EventStreamを経由せず、直接メールボックスに配送される
- **AND** システムメッセージとして通常メッセージより優先処理される

### Requirement: ActorContextにwatch/unwatchメソッドを追加する
`ActorContext`に`watch(target: &ActorRefGeneric<TB>)`と`unwatch(target: &ActorRefGeneric<TB>)`メソッドを追加する（MUST）。これらのメソッドは、対象アクターに`SystemMessage::Watch`/`Unwatch`を送信する。

#### Rationale
- Akka/PekkoのDeathWatch APIと互換性を持つ
- アクターロジック内で監視を明示的に制御できる

#### Scenario: watchメソッドで監視を開始できる
- **GIVEN** 親アクターが子アクターを監視したい場合
- **WHEN** `ctx.watch(child.actor_ref())?`を呼び出す
- **THEN** 子アクターに`SystemMessage::Watch(parent_pid)`が送信される
- **AND** 子アクターのwatchersリストに親のPidが追加される

#### Scenario: unwatchメソッドで監視を解除できる
- **GIVEN** 親アクターが子アクターの監視を解除したい場合
- **WHEN** `ctx.unwatch(child.actor_ref())?`を呼び出す
- **THEN** 子アクターに`SystemMessage::Unwatch(parent_pid)`が送信される
- **AND** 子アクターのwatchersリストから親のPidが削除される

#### Scenario: spawn_child_watchedメソッドで監視付き子アクターを生成できる
- **GIVEN** 親アクターが監視付きで子アクターを生成したい場合
- **WHEN** `ctx.spawn_child_watched(&props)?`を呼び出す
- **THEN** 子アクターが生成される
- **AND** 自動的に`watch()`が呼ばれる
- **AND** 子アクターのwatchersリストに親のPidが追加される

### Requirement: Actorトレイトにon_terminatedメソッドを追加する
`Actor`トレイトに`on_terminated(ctx: &mut ActorContext<'_, TB>, terminated: Pid) -> Result<(), ActorError>`メソッドを追加する（MUST）。このメソッドは、監視対象のアクターが停止したときに`ActorCell::handle_terminated()`から呼ばれる。デフォルト実装は`Ok(())`を返す。

#### Rationale
- Akka/Pekkoの`Terminated`メッセージハンドリングと同等の機能
- アクターロジック内で子の死活を処理できる
- `ActorCell::handle_terminated()`が`&mut ActorContext`を用意して呼び出すため、アクター処理コンテキスト内で実行される

#### Scenario: 監視対象が停止するとon_terminatedが呼ばれる
- **GIVEN** 親アクターが子アクターを監視している場合
- **WHEN** 子アクターが停止する
- **THEN** 親アクターのメールボックスに`SystemMessage::Terminated(child_pid)`が送信される
- **AND** `ActorCell::handle_terminated(child_pid)`が実行される
- **AND** `handle_terminated`内で`&mut ActorContext`が生成される
- **AND** 親アクターの`on_terminated(ctx, child_pid)`が呼ばれる
- **AND** 親アクターは子を再起動するなどの処理を実行できる

#### Scenario: unwatchした後はon_terminatedが呼ばれない
- **GIVEN** 親アクターが子アクターを監視している場合
- **WHEN** 親が`ctx.unwatch(child.actor_ref())?`を呼び出す
- **AND** その後、子アクターが停止する
- **THEN** 親アクターの`on_terminated`は呼ばれない
- **AND** watchersリストから削除されているため、`SystemMessage::Terminated`も送信されない

### Requirement: ActorCellはwatchersリストを管理する
`ActorCell`に`watchers: ToolboxMutex<Vec<Pid>, TB>`フィールドを追加し（MUST）、`handle_watch(watcher: Pid)`と`handle_unwatch(watcher: Pid)`メソッドで監視者の追加・削除を行う（MUST）。停止時に`notify_watchers_on_stop()`で各監視者に`SystemMessage::Terminated`を送信する（MUST）。

#### Rationale
- 監視者リストを効率的に管理
- アクター停止時に監視者への通知を自動化
- メモリリークを防ぐため、停止時にwatchersリストをクリア

#### Scenario: handle_watchで監視者を追加できる
- **GIVEN** `SystemMessage::Watch(watcher_pid)`を受信した場合
- **WHEN** `handle_watch(watcher_pid)`が呼ばれる
- **THEN** watchersリストにwatcher_pidが追加される
- **AND** 同じPidを複数回追加しても冪等（重複しない）

#### Scenario: handle_unwatchで監視者を削除できる
- **GIVEN** `SystemMessage::Unwatch(watcher_pid)`を受信した場合
- **WHEN** `handle_unwatch(watcher_pid)`が呼ばれる
- **THEN** watchersリストからwatcher_pidが削除される
- **AND** リストにいないPidを削除しても無害（エラーにならない）

#### Scenario: 停止時に全監視者にTerminatedを送信する
- **GIVEN** 監視者が2人いるアクターが停止する場合
- **WHEN** `notify_watchers_on_stop()`が呼ばれる
- **THEN** 各監視者のメールボックスに`SystemMessage::Terminated(self.pid)`が送信される
- **AND** 合計2つのTerminatedメッセージが送信される（メールボックス直接配送）
- **AND** 従来の`LifecycleEvent(stage: Stopped)`はEventStreamに発行される（システムワイド観測用）

#### Scenario: 停止時にwatchersリストをクリアする
- **GIVEN** アクターが停止する場合
- **WHEN** `stop()`処理が実行される
- **THEN** 監視者への通知後、watchersリストがクリアされる
- **AND** メモリリークを防止する

### Requirement: ActorCellにhandle_terminatedメソッドを追加する
`ActorCell`に`handle_terminated(terminated_pid: Pid) -> Result<(), ActorError>`メソッドを追加する（MUST）。このメソッドは`SystemMessage::Terminated`を受信したときに呼ばれ、`&mut ActorContext`を生成して`Actor::on_terminated()`を呼び出す（MUST）。

#### Rationale
- `on_terminated`の呼び出し元とコンテキスト生成の責務を明確化
- EventStreamサブスクライバーでは不可能だった`&mut ActorContext`の取得を実現
- アクター処理スレッド内での実行を保証

#### Scenario: handle_terminatedがActorContextを生成してon_terminatedを呼ぶ
- **GIVEN** 監視者アクターが`SystemMessage::Terminated(child_pid)`を受信した場合
- **WHEN** `ActorCell::handle_terminated(child_pid)`が呼ばれる
- **THEN** `ActorSystemGeneric::from_state(self.system.clone())`でシステムハンドルを取得する
- **AND** `ActorContext::new(&system, self.pid)`で`&mut ActorContext`を生成する
- **AND** `&mut self.actor`をロックする
- **AND** `actor.on_terminated(&mut ctx, child_pid)`を呼び出す
- **AND** アクターは`&mut ActorContext`を使って子を再起動するなどの処理を実行できる

#### Scenario: handle_terminatedはアクター処理コンテキスト内で実行される
- **GIVEN** `SystemMessage::Terminated`がシステムメッセージとして処理される場合
- **WHEN** `ActorCell::handle_terminated()`が呼ばれる
- **THEN** 通常のメッセージ処理と同じスレッド・コンテキスト内で実行される
- **AND** 順序保証が維持される（システムメッセージは通常メッセージより優先）
- **AND** `&mut self`への安全なアクセスが可能

### Requirement: SystemStateでWatch/Unwatch/Terminatedメッセージを処理する
`SystemState`の`process_system_message()`で`Watch`/`Unwatch`/`Terminated`メッセージを処理する（MUST）。特に、`Watch`メッセージ処理時に対象アクターが既に停止している場合、即座に`SystemMessage::Terminated`を監視者に送信する（MUST）。

#### Rationale
- Akka/Pekko互換の動作：既に停止したアクターをwatchすると即座にTerminatedが届く
- レースコンディションを回避：watch直後に停止しても通知を確実に受け取れる

#### Scenario: SystemMessage::Watchを処理する
- **GIVEN** アクターが`SystemMessage::Watch(watcher_pid)`を受信した場合
- **WHEN** `process_system_message()`が呼ばれる
- **THEN** 対象アクターの`ActorCell::handle_watch(watcher_pid)`が呼ばれる
- **AND** watchersリストに追加される

#### Scenario: SystemMessage::Unwatchを処理する
- **GIVEN** アクターが`SystemMessage::Unwatch(watcher_pid)`を受信した場合
- **WHEN** `process_system_message()`が呼ばれる
- **THEN** 対象アクターの`ActorCell::handle_unwatch(watcher_pid)`が呼ばれる
- **AND** watchersリストから削除される

#### Scenario: SystemMessage::Terminatedを処理する
- **GIVEN** アクターが`SystemMessage::Terminated(terminated_pid)`を受信した場合
- **WHEN** `process_system_message()`が呼ばれる
- **THEN** アクターの`ActorCell::handle_terminated(terminated_pid)`が呼ばれる
- **AND** `&mut ActorContext`が生成される
- **AND** `Actor::on_terminated()`が呼び出される

#### Scenario: 既に停止したアクターをwatchすると即座にTerminatedが送られる
- **GIVEN** アクターAが既に停止している場合
- **WHEN** アクターBが`ctx.watch(actorA_ref)?`を呼び出す
- **THEN** `SystemMessage::Watch(B_pid)`がアクターAに送信される
- **AND** `SystemState`がアクターAが既に停止していることを検出する
- **AND** 即座に`SystemMessage::Terminated(A_pid)`をアクターBに送信する
- **AND** アクターBの`on_terminated(ctx, A_pid)`が呼ばれる

#### Scenario: watch直後に停止してもTerminatedを受け取れる
- **GIVEN** アクターBがアクターAをwatchした直後
- **WHEN** アクターAが停止する（watchとstopがレース状態）
- **THEN** 以下のいずれかが保証される：
  - (1) watchersリストに追加された後に停止 → `notify_watchers_on_stop()`でTerminatedが送られる
  - (2) 停止した後にwatchが処理される → `SystemState`が即座にTerminatedを送る
- **AND** いずれのケースでもアクターBは必ずTerminatedを受け取る

### Requirement: no_std環境とstd環境の両方で動作する
全ての実装はRuntimeToolbox抽象化を通じてno_std環境（actor-core）とstd環境（actor-std）の両方で動作する（MUST）。

#### Rationale
- cellactor-rsは組み込みシステム対応のno_stdをサポート
- actor-stdはactor-coreを再エクスポートするため追加実装不要

#### Scenario: actor-coreでwatch/unwatchが使える
- **GIVEN** no_std環境のactor-coreを使用している場合
- **WHEN** `ctx.watch(child.actor_ref())?`を呼び出す
- **THEN** RuntimeToolboxの抽象化により正常に動作する
- **AND** no_std環境でコンパイル・実行できる

#### Scenario: actor-stdでwatch/unwatchが使える
- **GIVEN** std環境のactor-stdを使用している場合
- **WHEN** `ctx.watch(child.actor_ref())?`を呼び出す
- **THEN** actor-coreの実装がそのまま利用できる
- **AND** 追加の実装なしで動作する
