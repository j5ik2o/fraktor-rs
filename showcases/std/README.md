# std showcases

`fraktor-showcases-std` は、`std` 環境で実行できる fraktor-rs のサンプルコードを集約する crate です。
この README は、現在実装済みのサンプルと、今後網羅すべき未実装サンプルのインデックスです。

## 実行方法

通常の example は次の形式で実行します。

```bash
cargo run -p fraktor-showcases-std --example getting_started
```

`advanced` feature が必要な example は次の形式で実行します。

```bash
cargo run -p fraktor-showcases-std --features advanced --example remote_lifecycle
```

## サンプルインデックス

状態は、`showcases/std/Cargo.toml` に `[[example]]` として登録され、対応する `main.rs` が存在するものを `完了` としています。

| 状態 | サンプル | 対象領域 | 内容 | 実行コマンド |
|------|----------|----------|------|--------------|
| 完了 | `getting_started` | typed actor | typed API で actor system を作成し、guardian にメッセージを送信する最小構成 | `cargo run -p fraktor-showcases-std --example getting_started` |
| 完了 | `request_reply` | typed actor | `ask` と reply actor を使った request-reply パターン | `cargo run -p fraktor-showcases-std --example request_reply` |
| 完了 | `state_management` | typed actor | Behavior 遷移による immutable counter と state machine | `cargo run -p fraktor-showcases-std --example state_management` |
| 完了 | `child_lifecycle` | typed actor | 子 actor の生成、監視、終了通知、supervision restart | `cargo run -p fraktor-showcases-std --example child_lifecycle` |
| 完了 | `timers` | typed actor | one-shot timer、periodic timer、timer cancellation | `cargo run -p fraktor-showcases-std --example timers` |
| 完了 | `routing` | typed actor | pool router と round-robin による work distribution | `cargo run -p fraktor-showcases-std --example routing` |
| 完了 | `stash` | typed actor | `StashBuffer` による一時退避と unstash による再処理 | `cargo run -p fraktor-showcases-std --example stash` |
| 完了 | `serialization` | serialization | `Serializer` と `SerializerWithStringManifest` の登録と利用 | `cargo run -p fraktor-showcases-std --example serialization` |
| 完了 | `stream_pipeline` | stream | `Source`、`Map`、`FlatMapConcat`、`Fold`、`Sink` による stream pipeline | `cargo run -p fraktor-showcases-std --example stream_pipeline` |
| 完了 | `stream_authoring_apis` | stream | `GraphStage`、`GraphDsl`、`StreamRefs` などの stream authoring API | `cargo run -p fraktor-showcases-std --example stream_authoring_apis` |
| 完了 | `classic_logging` | kernel actor | kernel actor の logging facade、diagnostic logging、event stream 連携 | `cargo run -p fraktor-showcases-std --example classic_logging` |
| 完了 | `classic_timers` | kernel actor | kernel actor の timer API による single timer の起動と受信 | `cargo run -p fraktor-showcases-std --example classic_timers` |
| 完了 | `typed_event_stream` | typed actor | typed API から event stream へ subscribe / publish する流れ | `cargo run -p fraktor-showcases-std --example typed_event_stream` |
| 完了 | `typed_receptionist_router` | typed actor | `Receptionist` 登録と group router による service discovery routing | `cargo run -p fraktor-showcases-std --example typed_receptionist_router` |
| 完了 | `typed_async_first_actor_adapters` | typed actor | std Tokio helper、blocking dispatcher、typed `pipe_to_self` を組み合わせる async-first adapter サンプル | `cargo run -p fraktor-showcases-std --features advanced --example typed_async_first_actor_adapters` |
| 完了 | `remote_lifecycle` | remote | remote transport の起動、address 確認、shutdown、lifecycle event の観測 | `cargo run -p fraktor-showcases-std --features advanced --example remote_lifecycle` |
| 完了 | `persistent_actor` | persistence | `PersistentActor`、journal、snapshot store による event sourced actor | `cargo run -p fraktor-showcases-std --features advanced --example persistent_actor` |

## 未完了・追加候補

| 状態 | サンプル候補 | 対象領域 | 網羅したい内容 |
|------|--------------|----------|----------------|
| 未完了 | `remote_messaging` | remote | ネットワーク越しの actor 通信と message delivery |
| 未完了 | `cluster_membership` | cluster | cluster 参加、membership 変更、lifecycle event の観測 |
| 未完了 | `persistence_effector` | persistence | typed API から永続化 effector を扱うサンプル |

未完了候補を追加するときは、`showcases/std/<example-name>/main.rs` を作成し、`showcases/std/Cargo.toml` の `[[example]]` に登録してください。
