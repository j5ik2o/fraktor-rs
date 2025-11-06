## ADDED Requirements
### Requirement: Typed Behavior コアAPI
Rust 版 typed レイヤーは Behavior/Behaviors API を追加し、pekko の基本 DSL（same/stopped/ignore/receiveMessage/receiveSignal）と等価な語彙を提供しなければならない (MUST)。

#### Scenario: same/stopped/ignore のセンチネルを生成できる
- **WHEN** `Behaviors::same()` / `Behaviors::stopped()` / `Behaviors::ignore()` を呼び出す
- **THEN** それぞれ現在の挙動の維持、停止要求、シグナル・メッセージの無視を表す `Behavior<M>` 実装が得られる
- **AND** TypedActor 側からもこれらのセンチネルを区別できる

#### Scenario: receiveMessage/receiveSignal で Behavior を構築できる
- **WHEN** `Behaviors::receive_message` または `Behaviors::receive_signal` にクロージャを渡す
- **THEN** クロージャは `TypedActorContext` と入力値を受け取り、`Behavior` を返す
- **AND** クロージャが返す `Behavior` が次の状態としてランタイムに保存される

### Requirement: Behavior 遷移の実行と検証
Typed ランタイムは Behavior が返す次状態を評価し、pekko と同様の遷移規則を満たさなければならない (MUST)。

#### Scenario: receiveMessage で別 Behavior に差し替えられる
- **GIVEN** `Behaviors::receive_message` で mutable state を `Behaviors::receive_message` で包んだ挙動
- **WHEN** ハンドラが `Behaviors::receive_message` を再構築して返却する
- **THEN** 次のメッセージ以降は新しい Behavior が使用される

#### Scenario: Behavior::stopped を返すとアクターが停止する
- **WHEN** メッセージハンドラが `Behaviors::stopped()` を返す
- **THEN** Typed ランタイムはアクターに停止シグナルを送り、以降のメッセージは処理されない
- **AND** 単体テストがこの挙動を検証する

### Requirement: Behavior ベース example とテスト
新 API の利用方法は example と単体テストで示され、CI で自動検証されなければならない (MUST)。

#### Scenario: 単体テストで同一挙動と遷移を確認する
- **WHEN** テストが `Behaviors::same` と receiveMessage を使って状態を維持・更新する
- **THEN** 期待されるカウンタや停止フラグの値が検証される

#### Scenario: examples で Behavior ベースのアクターを実行できる
- **WHEN** `cargo run --example typed_behavior_*` を実行する
- **THEN** Behavior API を使ってメッセージを処理し、ログや print で状態遷移を観測できる
- **AND** README もしくは example ファイル内で実行方法が日本語コメントとして説明される
