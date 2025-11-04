## ADDED Requirements

### Requirement: Actor creationはActorContextに限定される
`ActorSystem` の公開 API からアクター操作メソッド（`spawn`, `spawn_child`, `actor_ref`, `children`, `stop_actor`）を削除し（MUST）、これらの操作は `ActorContext` を通じてのみ実行可能とする（MUST）。

#### Rationale
- アクター階層とスーパービジョン境界を保護
- Akka/Pekko Typedのベストプラクティスに準拠
- ランタイムの一貫性と安全性の向上

#### Scenario: トップレベルで spawn を呼びたい
- **GIVEN** アプリコードが `ActorSystem::spawn` を呼び出そうとした場合
- **THEN** その API は公開されておらずコンパイルエラーになる
- **AND** アクターは `ActorContext::spawn_child` で生成するようガイドされる

#### Scenario: トップレベルからアクターを動的に生成したい
- **GIVEN** ユーザーがランタイム中に新しいアクターを追加したい
- **WHEN** ガーディアンアクターにカスタムメッセージを送信する
- **THEN** ガーディアンが `ctx.spawn_child` で子アクターを生成できる
- **NOTE** このパターンはユーザーが自由に実装できる（MAY）

### Requirement: PIDベースのアクター参照は内部APIに制限される
任意の PID から `ActorRef` を取得する機能は、クレート内部に制限される（MUST）。外部ユーザーは、アクター間通信を通じて ActorRef を共有しなければならない（MUST）。

#### Scenario: PID から ActorRef を引きたい
- **GIVEN** 外部コードが `ActorSystem::actor_ref(pid)` を呼び出そうとする
- **THEN** その API は存在せず、代わりにアクターが自分の文脈内で参照を保持・共有する設計を要求される
- **NOTE** テスト用途では `#[cfg(test)]` での限定公開を検討できる（MAY）

### Requirement: アクター停止は親またはシステムシャットダウンから行う
任意の PID を直接停止する API は公開しない（MUST NOT）。アクターの停止は、親アクターによる管理またはシステム全体の終了処理を通じて行う（MUST）。

#### Scenario: 任意 PID に stop を送りたい
- **GIVEN** 外部コードが `ActorSystem::stop_actor(pid)` を呼び出したい
- **THEN** その API は公開されていない
- **AND** 停止は親アクターの管理またはシステム全体の `system.terminate()` で行う

### Requirement: テスト支援パターンを提供する
`#[cfg(test)]` 環境でトップレベル spawn を行いやすくするため、テスト用ガーディアンのパターン例を提供すべきである（SHOULD）。このヘルパーは crate-private 化した `ActorSystem` API を再公開してはならない（MUST NOT）。

#### Scenario: 単体テストで子アクターを作成したい
- **GIVEN** テストコードが guardian 経由で spawn した子アクターにアクセスしたい
- **THEN** テスト用ガーディアンが `on_start` で子アクターを生成し、参照を保持するパターンを使用できる
- **AND** `ActorSystem::actor_ref` に依存しない

### Requirement: ドキュメントとサンプルはActorContext経由のパターンを示す
公開ドキュメント・examples は `ActorContext::spawn_child` を使うコードに置き換えなければならない（MUST）。トップレベルからの動的生成が必要な場合、ガーディアンへのカスタムメッセージ送信パターンを例示できる（MAY）。

#### Scenario: ping_pong_tokio サンプル
- **GIVEN** `modules/actor-std/examples/ping_pong_tokio` を実行
- **THEN** guardian が `on_start` で必要なアクターを生成している
- **AND** README / ドキュメントにも同じ手順が記載されている
