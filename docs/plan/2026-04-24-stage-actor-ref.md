# StageActorRef 実装計画

## 目的
Pekko `GraphStageLogic.getStageActor` の契約を、Rust / fraktor-rs の stage authoring API と actor-core の temp actor delivery で表現する。

## 対象
- actor-core の `/temp` actor 宛て `SystemMessage::Watch` / `Unwatch` delivery
- stream-core の `StageActor` / `StageActorEnvelope` / `StageActorReceive`
- `StageContext::get_stage_actor` / `stage_actor`
- `ActorMaterializer` から graph stage flow context への actor system 注入

## 実装順序
1. actor-core の temp actor 解決と system message delivery を実装する。
2. stream-core の public stage actor 型を 1 公開型 1 ファイルで追加する。
3. `StageContext` と `GraphStageFlowContext` に stage actor state を接続する。
4. `GraphInterpreter` 起動時に `FlowLogic::on_start` を呼び、`ActorMaterializer` の actor system を注入する。
5. 追加済みテスト、対象 crate の型チェック、テスト、dylint を実行する。

## 除外
- remote transport 越しの location transparency
- StreamRef 本体
- TCP / TLS
- typed wrapper
