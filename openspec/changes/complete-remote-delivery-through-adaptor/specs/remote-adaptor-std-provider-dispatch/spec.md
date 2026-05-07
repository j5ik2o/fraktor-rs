## MODIFIED Requirements

### Requirement: actor_ref メソッドの remote 振り分け

`StdRemoteActorRefProvider::actor_ref` は actor-core provider surface の remote 分岐で、送信時に std remote event loop へ到達する `ActorRef` を返さなければならない（MUST）。

#### Scenario: リモート path の振り分けは配送経路に接続される

- **WHEN** `path` の authority が local address と一致しない状態で `StdRemoteActorRefProvider::actor_ref(path)` を呼ぶ
- **THEN** provider は remote path 用の `ActorRef` を返す
- **AND** その sender は `RemoteActorRefSender` 相当であり、`ActorRefSender::send` 呼び出し時に `RemoteEvent::OutboundEnqueued` を adapter 内部 sender に push する
- **AND** その event は `RemoteShared` event loop に処理され、対象 peer が接続済みかつ payload がサポート対象なら `TcpRemoteTransport::send` まで到達する

#### Scenario: provider dispatch から transport send までを contract test で確認する

- **WHEN** external integration test が actor-core provider surface から remote path を resolve し、サポート対象 payload を tell する
- **THEN** test は `RemoteEvent::OutboundEnqueued` の push だけでなく、`TcpRemoteTransport::send` が envelope frame を writer に enqueue したことまで観測する
- **AND** sender channel full / closed は caller が観測できる `SendError` に変換される
