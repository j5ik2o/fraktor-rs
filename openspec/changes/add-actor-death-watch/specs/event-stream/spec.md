## MODIFIED Requirements

### Requirement: EventStreamはwatcherフィールド付きLifecycleEventを配信する
EventStreamは、`watcher: Option<Pid>`フィールドを持つLifecycleEventを配信する（MUST）。既存のEventStreamEventには変更を加えず、Lifecycle variantのペイロードのみが拡張される。

#### Rationale
- EventStreamEventに新しいvariantを追加せず、既存の仕組みを活用
- システムワイド観測とDeathWatchの両方を統一的に扱う

#### Scenario: watcher: Noneのイベントは全サブスクライバーに配信される
- **GIVEN** アクターが停止した場合
- **WHEN** `LifecycleEvent { watcher: None, stage: Stopped }`が発行される
- **THEN** 全てのEventStreamサブスクライバーがイベントを受信する
- **AND** ロギング、メトリクス収集などのシステムワイド観測に使用できる

#### Scenario: watcher: Some(pid)のイベントは特定監視者向けに配信される
- **GIVEN** 監視者がいるアクターが停止した場合
- **WHEN** `LifecycleEvent { watcher: Some(watcher_pid), stage: Stopped }`が発行される
- **THEN** 全てのEventStreamサブスクライバーがイベントを受信する
- **AND** サブスクライバーは`event.watcher()`で自分宛てか判定できる
- **AND** `watcher_pid == my_pid`の場合のみ処理する

#### Scenario: 監視者がいる場合、両方のイベントが発行される
- **GIVEN** 監視者が2人いるアクターが停止した場合
- **WHEN** 停止処理が実行される
- **THEN** 監視者向けイベント × 2（各監視者のPid付き）が発行される
- **AND** システムワイド観測用イベント × 1（watcher: None）が発行される
- **AND** 合計3つのLifecycleEventがEventStreamに発行される

### Requirement: EventStreamサブスクライバーはwatcherフィールドでフィルタリングできる
EventStreamサブスクライバーは、`LifecycleEvent::watcher()`メソッドまたは`is_watched()`メソッドを使ってイベントをフィルタリングできる（MUST）。

#### Rationale
- 監視者向けイベントと通常イベントを明確に区別
- 不要なイベント処理を削減

#### Scenario: 監視者向けイベントのみを処理できる
- **GIVEN** EventStreamサブスクライバーがLifecycleEventを受信した場合
- **WHEN** `if let Some(watcher_pid) = event.watcher()`でフィルタリングする
- **THEN** 監視者向けイベントのみが処理される
- **AND** `watcher_pid == self.my_pid`の場合のみ自分宛てと判定できる

#### Scenario: システムワイドイベントのみを処理できる
- **GIVEN** EventStreamサブスクライバーがLifecycleEventを受信した場合
- **WHEN** `if event.watcher().is_none()`でフィルタリングする
- **THEN** システムワイド観測用イベントのみが処理される
- **AND** ロギングやメトリクス収集に使用できる

#### Scenario: is_watchedメソッドで簡潔にフィルタリングできる
- **GIVEN** EventStreamサブスクライバーがLifecycleEventを受信した場合
- **WHEN** `if event.is_watched()`で判定する
- **THEN** 監視者向けイベントかどうかを簡潔に判定できる
- **AND** コードの可読性が向上する

### Requirement: 既存のEventStreamサブスクライバーは互換性を保つ
既存のEventStreamサブスクライバーは、watcherフィールドを無視することで互換性を保つ（MUST）。パターンマッチングで`..`を使用すれば、新フィールドの影響を受けない。

#### Rationale
- 破壊的変更の影響を最小化
- 既存コードの移行を容易にする

#### Scenario: 既存のパターンマッチングは..で互換性を保つ
- **GIVEN** 既存のEventStreamサブスクライバーがある場合
- **WHEN** `match event { LifecycleEvent { pid, stage, .. } => { ... } }`のように記述する
- **THEN** watcherフィールドが無視される
- **AND** 既存コードは最小限の修正で動作する

#### Scenario: 便利メソッドを使えば互換性を保つ
- **GIVEN** 既存のLifecycleEvent生成コードがある場合
- **WHEN** `LifecycleEvent::new_stopped(pid, parent, name, timestamp)`に変更する
- **THEN** `watcher: None`が自動的に設定される
- **AND** 既存の動作が維持される
