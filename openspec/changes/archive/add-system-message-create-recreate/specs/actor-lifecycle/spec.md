## ADDED Requirements

### Requirement: SystemMessageにCreate/Recreateを追加する
`SystemMessage`列挙型に`Create`と`Recreate` variant を追加し、ActorCellがメールボックス経由で起動・再起動を処理できるようにする（MUST）。

#### Scenario: SystemMessage::Createで初期化する
- **GIVEN** `ActorSystem` が新しいアクターを spawn した
- **WHEN** ActorCell のシステムキューに `SystemMessage::Create` が enqueue される
- **THEN** ActorCell は `pre_start` を実行し、`LifecycleStage::Started` を発行する
- **AND** Create メッセージ処理中にエラーが起きた場合は spawn が失敗として報告される

#### Scenario: SystemMessage::Recreateで再起動する
- **GIVEN** Supervisor が子アクターの再起動を決定した
- **WHEN** 対象アクターに `SystemMessage::Recreate` が送信される
- **THEN** ActorCell は旧インスタンスに `post_stop` を呼び、インスタンスを再生成する
- **AND** その後 `pre_start` を `LifecycleStage::Restarted` で実行する
- **AND** 再起動が完了するまで通常メッセージは処理されない

### Requirement: spawn/restartフローはSystemMessageを経由する
`ActorSystem::spawn_with_parent` や `SystemState::handle_failure` など、起動・再起動を指示する箇所は `SystemMessage::Create` / `SystemMessage::Recreate` を enqueue する実装に統一する（MUST）。

#### Scenario: spawnでCreateメッセージが送信される
- **GIVEN** 親アクターが子を生成した
- **WHEN** ActorCell が初期化される
- **THEN** 親フローは子の mailbox に `SystemMessage::Create` を enqueue する
- **AND** ActorCell はメールボックス処理中にのみ `pre_start` を実行する

#### Scenario: SupervisorがRecreateを送信する
- **GIVEN** 子アクターが Recoverable エラーを返し Supervisor が Restart を選択した
- **WHEN** `SystemState::handle_failure` が再起動対象を決定する
- **THEN** 再起動対象ごとに `SystemMessage::Recreate` が enqueue される
- **AND** 子アクターはメールボックス上で再起動処理を完了させる

### Requirement: Lifecycle hookの順序を保証する
`pre_start` / `post_stop` / `LifecycleStage` 発火は Create/Recreate の処理内でのみ実行され、他経路から重複実行されないようにする（MUST）。

#### Scenario: pre_startはCreate経路からのみ呼ばれる
- **GIVEN** ActorCell が Create を受信した
- **WHEN** `pre_start` が成功する
- **THEN** 同一アクターに対して他の箇所から `pre_start` が呼ばれることはない

#### Scenario: 再起動時の順序を維持する
- **GIVEN** `SystemMessage::Recreate` が処理される
- **WHEN** ActorCell が再起動を終える
- **THEN** `post_stop` → インスタンス再生成 → `pre_start` → `LifecycleStage::Restarted` の順序が保証される

### Requirement: Create完了前の状態を制御する
Create が完了するまでは spawn の呼び出し結果を確定させず、通常メッセージ処理も開始しない（MUST）。

#### Scenario: spawnはCreate結果を待ってから成功を返す
- **GIVEN** 親アクターが子アクターを spawn する
- **WHEN** `SystemMessage::Create` が mailbox に enqueue される
- **THEN** dispatcher が `pre_start` を成功させるまで `ActorSystem::spawn_with_parent` は成功を返さない
- **AND** `pre_start` が失敗または panic した場合は SpawnError として呼び出し元へ返却される

#### Scenario: Create完了まではユーザーメッセージを処理しない
- **GIVEN** 親アクターが子アクターを spawn した直後に通常メッセージを送る
- **WHEN** 子アクターの mailbox で `SystemMessage::Create` と通常メッセージが待機している
- **THEN** mailbox は system queue を優先し、`pre_start` が完了するまで通常メッセージを dispatch しない

### Requirement: Create/Recreate送信失敗時の一貫性
SystemMessage の enqueue に失敗した場合でも、システム状態が不整合にならないよう rollback や Escalate を実行する（MUST）。

#### Scenario: Create送信に失敗したらspawnを巻き戻す
- **GIVEN** ActorCell の作成には成功したが mailbox が閉じている
- **WHEN** `SystemMessage::Create` の送信が失敗する
- **THEN** `rollback_spawn` が呼ばれ PID 登録が取り消される
- **AND** `ActorSystem::spawn_with_parent` は SpawnError を返す

#### Scenario: Recreate送信に失敗したらStopへフォールバックする
- **GIVEN** Supervisor が Restart を選択した
- **WHEN** `SystemMessage::Recreate` の送信が失敗する
- **THEN** 対象 PID に `SystemMessage::Stop` を送信し、Supervisor へ Escalate する
- **AND** 監督ツリーが停止または再評価され、一貫した状態に戻る
