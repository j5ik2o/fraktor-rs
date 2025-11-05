## MODIFIED Requirements

### Requirement: LifecycleEventはwatcherフィールドを持つ
`LifecycleEvent`構造体に`watcher: Option<Pid>`フィールドを追加する（MUST）。このフィールドは、イベントが特定の監視者向けか（`Some(pid)`）、システムワイド観測用か（`None`）を区別する。

#### Rationale
- 単一イベント型で監視者向けと通常イベントの両方を表現（DRY原則）
- EventStreamEventに新しいvariantを追加する必要がない
- Akka/PekkoのDeathWatchと同等の機能を提供

#### Scenario: アクター停止時にwatcherフィールドがNoneのイベントが発行される
- **GIVEN** 監視者がいないアクターが停止した場合
- **WHEN** アクターが停止処理を実行する
- **THEN** `LifecycleEvent { stage: Stopped, watcher: None }`がEventStreamに発行される
- **AND** 全てのEventStreamサブスクライバーがこのイベントを受信する

#### Scenario: アクター停止時にwatcherフィールドがSomeのイベントが発行される
- **GIVEN** 監視者が1人以上いるアクターが停止した場合
- **WHEN** アクターが停止処理を実行する
- **THEN** 各監視者向けに`LifecycleEvent { stage: Stopped, watcher: Some(watcher_pid) }`が発行される
- **AND** システムワイド観測用に`LifecycleEvent { stage: Stopped, watcher: None }`も発行される

#### Scenario: is_watchedメソッドで監視者向けイベントを判定できる
- **GIVEN** LifecycleEventを受信した場合
- **WHEN** `event.is_watched()`を呼び出す
- **THEN** `watcher.is_some()`の結果が返される
- **AND** 監視者向けイベントと通常イベントを明確に区別できる

### Requirement: LifecycleEvent便利メソッドを提供する
既存コードの破壊的変更を最小化するため、`new_started()`, `new_stopped()`, `new_restarted()`便利メソッドを提供する（MUST）。これらのメソッドは`watcher: None`をデフォルトで設定する。

#### Rationale
- 既存コードの移行を容易にする
- 直接構造体を生成するコードの修正を最小限にする

#### Scenario: 便利メソッドでシステムワイドイベントを生成できる
- **GIVEN** LifecycleEventを生成する必要がある場合
- **WHEN** `LifecycleEvent::new_stopped(pid, parent, name, timestamp)`を呼び出す
- **THEN** `watcher: None`が自動的に設定されたイベントが生成される
- **AND** 既存コードは最小限の修正で動作する

#### Scenario: 監視者向けイベント生成用の専用メソッドを提供する
- **GIVEN** 監視者向けに終了通知イベントを生成する必要がある場合
- **WHEN** `LifecycleEvent::new_terminated(pid, parent, name, timestamp, watcher)`を呼び出す
- **THEN** `watcher: Some(watcher_pid)`が設定されたイベントが生成される
- **AND** `stage`は`LifecycleStage::Stopped`となる

## ADDED Requirements

### Requirement: ActorContextにwatch/unwatchメソッドを追加する
`ActorContext`に`watch(target: &ActorRef)`と`unwatch(target: &ActorRef)`メソッドを追加する（MUST）。これらのメソッドは、対象アクターにSystemMessage::Watch/Unwatchを送信する。

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
`Actor`トレイトに`on_terminated(ctx: &mut ActorContext, terminated: Pid)`メソッドを追加する（MUST）。このメソッドは、監視対象のアクターが停止したときに呼ばれる。デフォルト実装は何もしない。

#### Rationale
- Akka/Pekkoの`Terminated`メッセージハンドリングと同等の機能
- アクターロジック内で子の死活を処理できる

#### Scenario: 監視対象が停止するとon_terminatedが呼ばれる
- **GIVEN** 親アクターが子アクターを監視している場合
- **WHEN** 子アクターが停止する
- **THEN** 親アクターの`on_terminated(ctx, child_pid)`が呼ばれる
- **AND** 親アクターは子を再起動するなどの処理を実行できる

#### Scenario: unwatchした後はon_terminatedが呼ばれない
- **GIVEN** 親アクターが子アクターを監視している場合
- **WHEN** 親が`ctx.unwatch(child.actor_ref())?`を呼び出す
- **AND** その後、子アクターが停止する
- **THEN** 親アクターの`on_terminated`は呼ばれない
- **AND** watchersリストから削除されているため、通知イベントも発行されない

### Requirement: ActorCellはwatchersリストを管理する
`ActorCell`に`watchers: ToolboxMutex<Vec<Pid>>`フィールドを追加し（MUST）、`handle_watch()`と`handle_unwatch()`メソッドで監視者の追加・削除を行う（MUST）。停止時に`notify_watchers_on_stop()`で各監視者にイベントを発行する（MUST）。

#### Rationale
- 監視者リストを効率的に管理
- アクター停止時に監視者への通知を自動化

#### Scenario: handle_watchで監視者を追加できる
- **GIVEN** SystemMessage::Watch(watcher_pid)を受信した場合
- **WHEN** `handle_watch(watcher_pid)`が呼ばれる
- **THEN** watchersリストにwatcher_pidが追加される
- **AND** 同じPidを複数回追加しても冪等（重複しない）

#### Scenario: handle_unwatchで監視者を削除できる
- **GIVEN** SystemMessage::Unwatch(watcher_pid)を受信した場合
- **WHEN** `handle_unwatch(watcher_pid)`が呼ばれる
- **THEN** watchersリストからwatcher_pidが削除される
- **AND** リストにいないPidを削除しても無害（エラーにならない）

#### Scenario: 停止時にwatchersリストをクリアする
- **GIVEN** アクターが停止する場合
- **WHEN** `stop()`処理が実行される
- **THEN** 監視者への通知後、watchersリストがクリアされる
- **AND** メモリリークを防止する

### Requirement: SystemMessageにWatch/Unwatchを追加する
`SystemMessage`列挙型に`Watch(Pid)`と`Unwatch(Pid)`variantを追加する（MUST）。SystemStateでこれらのメッセージを処理し、対象アクターの`handle_watch()`/`handle_unwatch()`を呼び出す（MUST）。

#### Rationale
- アクター間通信の既存インフラを活用
- システムメッセージとして優先的に処理

#### Scenario: SystemMessage::Watchを処理できる
- **GIVEN** アクターがSystemMessage::Watch(watcher_pid)を受信した場合
- **WHEN** SystemStateがメッセージを処理する
- **THEN** 対象アクターの`handle_watch(watcher_pid)`が呼ばれる
- **AND** watchersリストに追加される

#### Scenario: SystemMessage::Unwatchを処理できる
- **GIVEN** アクターがSystemMessage::Unwatch(watcher_pid)を受信した場合
- **WHEN** SystemStateがメッセージを処理する
- **THEN** 対象アクターの`handle_unwatch(watcher_pid)`が呼ばれる
- **AND** watchersリストから削除される

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
