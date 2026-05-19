## MODIFIED Requirements

### Requirement: shutdown_and_join による wake + 完了観測（&self、握りつぶし禁止に従う）

run task の graceful flush、wake、完了観測を 1 step で行う adapter 固有の async API `RemotingExtensionInstaller::shutdown_and_join(&self) -> impl Future<Output = Result<(), RemotingError>>` を提供する SHALL。**`&self` を取る**（`self` consume ではない、`ExtensionInstaller` で actor system に登録されたまま使えるようにする）。`RemoteShared::shutdown` は **wake せず**、`event_sender` を持たない（薄いラッパー原則）。

`shutdown_and_join` は active association の shutdown flush 完了または timeout を待ってから `RemoteShared::shutdown` を呼ばなければならない（MUST）。shutdown flush の送信失敗または timeout は観測可能に記録しなければならないが（MUST）、transport shutdown と run task join を永久に止めてはならない（MUST NOT）。

shutdown_and_join 内では must-use 戻り値を `let _ = ...` で握りつぶしてはならない（MUST NOT、`.agents/rules/ignored-return-values.md` 準拠）。失敗の意味分類と扱いを明示する。

#### Scenario: 同期 Remoting::shutdown の挙動（純デリゲートのみ）

- **WHEN** `Remoting::shutdown`（`RemoteShared::shutdown` 経由）が呼ばれる
- **THEN** `with_write(|remote| remote.shutdown())` で `Remote::shutdown` を呼び lifecycle を terminated に遷移する
- **AND** **wake はしない**（`RemoteShared` は `event_sender` を持たない）
- **AND** `event_sender.send(...).await` や `event_sender.try_send(...)` や `run_handle.await` を内部で実行しない（`RemoteShared` 自体が adapter 都合の field を持たないため）
- **AND** 同期 method なので `await` を内部で行わない

#### Scenario: shutdown_and_join の手順（flush first、&self、内部可変性経由）

- **WHEN** `installer.shutdown_and_join(&self).await` が呼ばれる
- **THEN** installer は次の 5 ステップを順次実行する
  1. `remote_shared`、`event_sender`、flush timeout、run state を `&self` の内部可変性 field から取得する
  2. active association がある場合、association outbound queue を drain したうえで scope `Shutdown` の flush を request し、すべての flush completed または `shutdown_flush_timeout` まで待つ
  3. `self.remote_shared.get().ok_or(RemotingError::NotStarted)?.shutdown()` を呼ぶ（lifecycle terminated 遷移、`RemoteShared::shutdown` の純デリゲート。既に停止要求済みまたは停止済みなら no-op `Ok(())`）
  4. `self.event_sender.get()` で `Sender` を取得し、`if let Err(send_err) = sender.try_send(RemoteEvent::TransportShutdown) { tracing::debug!(?send_err, "shutdown wake failed (best-effort)"); }` で wake（Full / Closed 失敗は log 記録。`TransportShutdown` handler は既に停止要求済み/停止済みなら no-op）
  5. `self.run_handle.lock().map_err(...)?.take()` で `Option<JoinHandle>` から handle を取り出し、`Some(handle)` なら `handle.await` で run task の終了を観測する
- **AND** ステップ 5 の `Result<Result<(), RemotingError>, JoinError>` を `match` で全分岐扱い `Ok(Ok(())) → Ok(())` / `Ok(Err(e)) → Err(e)` / `Err(join_err) → tracing::error!(?join_err, "...") + Err(RemotingError::TransportUnavailable)`

#### Scenario: no active association skips flush wait

- **WHEN** `shutdown_and_join(&self).await` が呼ばれ、active association が存在しない
- **THEN** installer は shutdown flush request を送らない
- **AND** `RemoteShared::shutdown`、wake、join の手順へ進む

#### Scenario: shutdown flush timeout still shuts down

- **WHEN** shutdown flush が `shutdown_flush_timeout` までに完了しない
- **THEN** timeout は log または test-observable path に記録される
- **AND** installer は `RemoteShared::shutdown`、wake、join の手順へ進む

#### Scenario: must-use 戻り値の握りつぶし禁止に従う

- **WHEN** `shutdown_and_join` 実装の `Result` 戻り値の扱いを検査する
- **THEN** `let _ = self.remote_shared...shutdown();` のような無言握りつぶしが存在しない
- **AND** flush request / wait の `Result` は `match` または `?` で扱い、timeout / failure は log または returned outcome に残す
- **AND** ステップ 3 の `Result` は `?` または `match` で扱い、`Err` を idempotent として握りつぶす分岐は存在しない（既に停止要求済み/停止済みなら `RemoteShared::shutdown` 自体が no-op `Ok(())` を返す）
- **AND** ステップ 4 の `try_send` の `Result` は `if let Err(send_err) = ...` で error 値を log に渡す
- **AND** ステップ 5 の `JoinHandle::await` の `Result` は `match` で全分岐を扱い、`Err(JoinError)` は log 記録 + `RemotingError::TransportUnavailable` 変換

#### Scenario: try_send の Full / Closed の意味分類

- **WHEN** `event_sender.try_send(RemoteEvent::TransportShutdown)` が `Err(TrySendError::Full)` を返す
- **THEN** event channel が満杯であり、現在 receiver は未消費 event を保持している
- **AND** その未消費 event の処理後 `RemoteShared::run` が `is_terminated()` Query で `true` を観測してループ終了する
- **AND** ステップ 5 の `handle.await` で完了を観測できるため、wake 失敗は best-effort として log 記録に留める
- **WHEN** `event_sender.try_send(RemoteEvent::TransportShutdown)` が `Err(TrySendError::Closed)` を返す
- **THEN** receiver は既に drop しており、`RemoteShared::run` task は既に終了している（`EventReceiverClosed` 経由で）
- **AND** ステップ 5 の `handle.await` で `Err(EventReceiverClosed)` または既終了結果が観測されるため、wake 失敗は best-effort として log 記録に留める

#### Scenario: RemoteShared::run の異常終了の観測

- **WHEN** `RemoteShared::run` が `Err(RemotingError::TransportUnavailable)` 等を返した
- **THEN** `installer.shutdown_and_join(&self).await` のステップ 5 の戻り値で error が呼出元に伝播される
- **AND** adapter は必要に応じて log 記録 / actor system の error path への通知を行う

#### Scenario: shutdown_and_join 単独でも完了する

- **WHEN** 呼び出し側が事前に `Remoting::shutdown` を呼ばずに `installer.shutdown_and_join(&self).await` だけを呼ぶ
- **THEN** ステップ 2 の shutdown flush wait、ステップ 3 の `RemoteShared::shutdown`、ステップ 4 の wake、ステップ 5 の完了観測が完結する
- **AND** 結果として graceful shutdown が成立する（呼び出し側が手順を意識する必要がない）

#### Scenario: 別 Driver 型の不在

- **WHEN** adapter 側の installer 実装を検査する
- **THEN** `RemoteDriverHandle` や `RemoteDriverOutcome` を import / 利用していない

## ADDED Requirements

### Requirement: remote-bound DeathWatch notification waits for flush outcome

std flush gate は remote watch hook から渡された remote-bound `DeathWatchNotification` を送る前に、対象 association の `BeforeDeathWatchNotification` flush を開始し、flush completed / timed out / failed のいずれかを観測してから notification envelope を enqueue する SHALL。remote-bound notification の発生点は `StdRemoteWatchHook::handle_deathwatch_notification` であり、`WatcherState` の heartbeat / failure detector 経路ではない。

#### Scenario: notification is delayed until flush completes

- **WHEN** remote watch hook が remote-bound `DeathWatchNotification` を std flush gate に渡す
- **THEN** flush gate は notification envelope を pending map に保持する
- **AND** 対象 association に `BeforeDeathWatchNotification` flush を request する
- **AND** flush completed を観測するまで notification envelope を enqueue しない

#### Scenario: timeout releases pending notification

- **GIVEN** remote-bound `DeathWatchNotification` が flush completion を待っている
- **WHEN** flush timeout を観測する
- **THEN** flush gate は timeout を log または test-observable path に記録する
- **AND** pending notification envelope を system priority envelope として enqueue する

#### Scenario: flush start failure releases pending notification

- **WHEN** flush gate が `BeforeDeathWatchNotification` flush を開始できない
- **THEN** failure を log または test-observable path に記録する
- **AND** notification envelope を破棄せず、system priority envelope として enqueue する

#### Scenario: completed flush enqueues exactly once

- **GIVEN** remote-bound `DeathWatchNotification` が flush completion を待っている
- **WHEN** flush completed event を複数回観測する
- **THEN** flush gate は notification envelope を一度だけ enqueue する

### Requirement: flush outcomes are applied after core event steps

std run loop は core event step が発生させた flush completed / timed-out / failed outcome を event step 後処理として std waiter / flush gate へ渡す SHALL。std waiter wake、pending notification release、actor-core enqueue の実行中に `Remote` の write lock を保持してはならない（MUST NOT）。

`RemoteTransport` は現行設計上 `Remote` が所有するため、flush request の transport 送信は `Remote::handle_remote_event` / `RemoteShared` の write lock 内で実行してよい。ただしこの transport method は bounded return / non-reentry 制約を守り、std の async wait や actor-core delivery を行ってはならない（MUST NOT）。

#### Scenario: flush request effect is sent through transport

- **WHEN** core event step が flush request effect を返す
- **THEN** core は `RemoteTransport` の lane-targeted flush request method へ flush request control frames を渡す
- **AND** send failure は log または returned error path に残す
- **AND** transport method は async wait、actor-core delivery、`RemoteShared` 再入を行わない

#### Scenario: flush completion wakes waiting shutdown

- **WHEN** core event step が shutdown flush completed effect を返す
- **THEN** std run loop は write lock を解放した後に `shutdown_and_join` の flush waiter を起こす
- **AND** waiter は `RemoteShared::shutdown` へ進める

#### Scenario: DeathWatch flush outcome wakes flush gate

- **WHEN** core event step が `BeforeDeathWatchNotification` flush completed または timed-out effect を返す
- **THEN** std run loop は write lock を解放した後に std flush gate へ flush outcome を渡す
- **AND** flush gate は pending notification の enqueue 判定を行う
