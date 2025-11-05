## ADDED Requirements

### Requirement: SystemMessageにFailureを追加する
`SystemMessage` 列挙に `Failure` variant を追加し、PID・原因・再起動統計・直前メッセージを mailbox 経由で上流へ渡せるようにする（MUST）。

#### Scenario: Failureメッセージで原因を伝搬する
- **GIVEN** 子アクターがユーザーメッセージ処理中に `ActorError` を返した
- **WHEN** ランタイムが失敗を検知する
- **THEN** 親のメールボックスに `SystemMessage::Failure` が enqueue され、PID/原因/直前メッセージ/再起動統計が含まれている

#### Scenario: Failureはユーザーメッセージより優先される
- **GIVEN** 子アクターが failure を発生させ、その後に通常メッセージが親に届いた
- **WHEN** 親の mailbox が dequeues を行う
- **THEN** `SystemMessage::Failure` が先に処理され、監督処理が完了するまで通常メッセージは実行されない

### Requirement: Failure経路で監督/エスカレーションを統一する
Failure system message が届いた親は監督戦略を実行し、Restart/Stop/Escalate の結果を mailbox 経由で反映しなければならない（MUST）。

#### Scenario: Failure受信で監督戦略を起動する
- **GIVEN** 親アクターが `SupervisorStrategy::OneForOne` を使用している
- **WHEN** 親が `SystemMessage::Failure` を受け取る
- **THEN** 親は再起動統計を更新し、戦略の判断（Restart/Stop/Escalate）を行い、対象 PID へ適切な system message（Recreate/Stop/Escalate 先への Failure）を送る

#### Scenario: Failureエスカレーションはルート監督にも届く
- **GIVEN** 親が Escalate を選択した
- **WHEN** 親は Failure をさらに上位へ転送する
- **THEN** ルート監督/guardian が Failure を受け取り、停止またはシステム終了を決定する

### Requirement: Failure送信はメトリクスとイベントを発行する
Failure が生成・処理されるたびに、メトリクスや EventStream に失敗イベントを publish し、監視ツールがフローを観測できるようにする（MUST）。

#### Scenario: Failure生成時にメトリクスを増分する
- **GIVEN** ActorSystem で内部メトリクス収集が有効になっている
- **WHEN** ランタイムが `SystemMessage::Failure` を enqueue する
- **THEN** 失敗カウンタをインクリメントし、PID/原因種別を属性として付与する

#### Scenario: Failure完了時にEventStreamへ通知する
- **GIVEN** Failure を処理した結果として Restart / Stop / Escalate のいずれかが決定した
- **WHEN** 監督戦略がその結果を実行し終える
- **THEN** EventStream に「Failure→監督結果」のシーケンスが publish され、デバッグ/モニタリングから辿れる
