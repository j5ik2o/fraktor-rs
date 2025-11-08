# pipe_to_self Future 設計メモ

## 背景

- 現状の `ActorContextGeneric::pipe_to_self` は「任意メッセージを同期クロージャで即時変換して `self_ref().tell` へ渡す」だけで、Future完了通知やタスクライフサイクル管理を持たない。
- `TypedActorContextGeneric::pipe_to_self` も `AdaptMessage` を包むだけの薄いラッパーで、pekko/akka の `pipeToSelf` にある「Future完了 → 結果マッピング → 自分宛てメッセージ化」の体験を再現できていない。
- HTTPクライアントなど `async` API と連携する際、Actor トレイトを `async fn` にできない制約が残っており、Future完了を安全に受け取る仕組みが求められている。

## 目標

1. Actor トレイトを同期のまま保ちつつ、任意の `Future` 完了をアクター自身へ配送できるようにする。
2. Future 結果のマッピングは常にアクターのスレッド（=メールボックス処理スレッド）上で実行し、pekko の `pipeToSelf` と同等のメモリ安全性を確保する。
3. `MessageAdapterRegistry`／`AdaptMessage` の仕組みを再利用し、ask/pipe 系のレスポンス変換を一元化する。
4. アクター停止時には保留中の pipe タスクを確実に `abort` し、DeadLetter に不要なノイズを発生させない。

## コア設計

### ActorContextGeneric の責務拡張

- `ActorContextGeneric<'a, TB>` に `pipe_handles: Vec<PipeHandle>` 相当のフィールドを追加し、`RuntimeToolbox::spawn` で起動した Future の `AbortHandle` を登録・破棄できるようにする。
- `pipe_to_self` API は `Future` を受け取り、完了時に `ContextPipeMessage`（新規メッセージ型）を自分宛てに `tell` するタスクを生成する。
- `PipeHandle` は `ActorCell` の停止フロー（`post_stop`, `pre_restart`）から呼び出せる `drop_pipe_handles` で一括 `abort` する。

### ContextPipeMessage と AdaptMessage

- Future 完了後はまず `ContextPipeMessage { entry_id, outcome }` として Untyped メールボックスに投入する。
- Typed 側では `AdaptMessage::<M, TB>` を流用し、`ContextPipeMessage` が到着した際に `AdapterEntry` を実行して `M` へ変換する。これにより ask/pipe 経路を同じ AdapterRegistry が処理できる。
- `entry_id`（もしくは `ReqId`）を使って複数同時 pipe を区別し、結果メッセージと元リクエストを結び付ける。

### Future 実行と cancellable spawn

1. `pipe_to_self(fut, map_ok, map_err)` 呼び出しで `fut` を `RuntimeToolbox::spawn` に登録。
2. Future 完了後に `PipeOutcome::Ok(T)` または `PipeOutcome::Err(E)` を生成し、`ContextPipeMessage` に格納。
3. `ActorContextGeneric` が保持する `FunctionRef`（既存の message adapter 経路）を通じて自分のメールボックスへ `tell`。
4. メールボックスで `ContextPipeMessage` を受信したアクターが `map_ok/map_err` をアクター・スレッド上で実行し、通常の `receive` ルートで結果メッセージを処理する。
5. アクター停止時には `AbortHandle` で Future をキャンセルし、結果は破棄される。

## API 案

### Untyped

```rust
impl<'a, TB> ActorContextGeneric<'a, TB> {
  pub fn pipe_to_self<Fut, Map>(
    &self,
    fut: Fut,
    map: Map,
  ) -> Result<(), PipeSpawnError>
  where
    Fut: Future<Output = PipeResult> + Send + 'static,
    Map: FnOnce(PipeOutcome) -> AnyMessageGeneric<TB> + Send + 'static;
}
```

- 既存の「同期メッセージをそのまま戻す」用途は `pipe_message_to_self(message)` のような別メソッドへ分離して後方互換を取りつつ段階的に移行。
- `PipeResult` は `Result<AnyMessageGeneric<TB>, PipeFailure>` を想定。`PipeFailure` には `TargetStopped` / `Cancelled` / `AdapterError` などを定義。

### Typed

```rust
impl<'a, M, TB> TypedActorContextGeneric<'a, M, TB> {
  pub fn pipe_to_self<U, Fut, MapOk, MapErr>(
    &mut self,
    fut: Fut,
    map_ok: MapOk,
    map_err: MapErr,
  ) -> Result<(), PipeSpawnError>
  where
    Fut: Future<Output = Result<U, E>> + Send + 'static,
    MapOk: FnOnce(U) -> Result<M, AdapterFailure> + Send + 'static,
    MapErr: FnOnce(E) -> Result<M, AdapterFailure> + Send + 'static;
}
```

- 内部では `AdaptMessage::<M, TB>::new(PipeOutcome, adapter)` を生成し、Untyped `pipe_to_self` を呼び出す。Typed API 利用者は純粋に `map_ok/map_err` でメッセージ変換を記述するだけで済む。

## エラーハンドリングと DeadLetter

- `self_ref().tell` が `SendError::TargetStopped` を返した場合、`PipeFailure::TargetStopped` としてラップし DeadLetter へ記録。
- Adapter 実行中の panic / `AdapterFailure` は `PipeOutcome::Err` に変換し、開発者に通知できるよう標準化。
- Future 側で `tokio::spawn` などが `JoinError` を返した場合は `PipeFailure::TaskPanic` を生成し、メトリクスやログと紐づける。

## テスト戦略

1. **happy path**: Future 完了後に `map_ok` がアクター・スレッドで一度だけ実行され、メールボックス順序が保たれる。
2. **cancel path**: アクター停止時に `drop_pipe_handles` が走り、Future が `Cancelled` になる。
3. **error propagation**: Future `Err` → `map_err` が呼ばれる／AdapterFailure が DeadLetter に記録される。
4. **concurrency**: 複数同時 pipe が `ReqId` で区別され、完了順序に応じてメッセージ化される。
5. **typed/untyped parity**: Untyped 経路と Typed ラッパーが同じ ContextPipeMessage を共有し、ask 経路とも干渉しないことを検証。

## 使用例

### Untyped Actor での利用例

```rust
use alloc::{boxed::Box, sync::Arc};
use cellactor_actor_core_rs::{
  actor_prim::{Actor, ActorContext, ActorContextGeneric, actor_ref::ActorRef},
  error::ActorError,
  messaging::AnyMessage,
};
use cellactor_utils_core_rs::sync::NoStdToolbox;

struct FetcherActor;

impl Actor for FetcherActor {
  fn receive(
    &mut self,
    ctx: &mut ActorContext<'_>,
    message: AnyMessage<NoStdToolbox>,
  ) -> Result<(), ActorError> {
    let request = message.downcast_ref::<Arc<str>>().expect("string payload").clone();
    ctx.pipe_to_self(
      async move {
        // ここではスタブ化したHTTP呼び出しを想定
        let body = fake_http(request.as_ref()).await;
        AnyMessage::new(body)
      },
      |response| response,
    )?;

    Ok(())
  }
}

async fn fake_http(_path: &str) -> String {
  "ok".to_string()
}
```

ポイント:

- `ctx.pipe_to_self` に Future と戻り値をメッセージへ変換するクロージャを渡すだけでよい。
- Future 内で HTTP クライアント等の `async` API をそのまま利用できる。
- Future 完了後の `AnyMessage` は自分自身のメールボックスへ投入され、通常の `receive` ルートで処理できる。

### Typed Behavior での利用例

```rust
use cellactor_actor_core_rs::{
  typed::{
    Behaviors,
    actor_prim::{TypedActorContext, TypedActorRef},
    behavior::Behavior,
  },
  NoStdToolbox,
};

#[derive(Clone)]
enum Command {
  Fetch(Arc<str>),
  Fetched(String),
  Failed(String),
}

fn fetcher_behavior() -> Behavior<Command, NoStdToolbox> {
  Behaviors::receive_message(|ctx, msg| match msg {
    Command::Fetch(path) => {
      ctx.pipe_to_self(
        async move { fake_http(path.as_ref()).await.map(Command::Fetched).map_err(|e| e.to_string()) },
        |payload| Ok(payload),
        |error| Ok(Command::Failed(error)),
      )?;
      Ok(Behaviors::same())
    },
    Command::Fetched(body) => {
      ctx.log(LogLevel::Info, format!("fetched: {body}"));
      Ok(Behaviors::same())
    },
    Command::Failed(reason) => {
      ctx.log(LogLevel::Warn, format!("failed: {reason}"));
      Ok(Behaviors::same())
    },
  })
}

async fn fake_http(_path: &str) -> Result<String, &'static str> {
  Ok("ok".to_string())
}
```

ポイント:

- `map_ok` と `map_err` に `Result<T, E>` → `Result<M, AdapterFailure>` 変換を記述するだけで、成功・失敗両方のメッセージハンドリングを型安全に統合できる。
- `fake_http` が返すエラーを `Command::Failed` にマップし、同じアクター内でエラーログを出せる。
- `Behavior` 側は同期のまま保てるため、`pipe_to_self` 呼び出し以外に `async` 境界を意識する必要がない。

## 移行ステップ

1. `ActorContextGeneric` に pipe タスク管理のフィールドと drop フローを追加。
2. Untyped API を Future ベースへ差し替え、既存の同期ショートカット用のラッパーを用意。
3. `AdaptMessage`／`MessageAdapterRegistry` を `ContextPipeMessage` 入力に対応させる。
4. `TypedActorContextGeneric::pipe_to_self` を新APIで実装し直し、ask ヘルパとも共有する。
5. ドキュメントと changelog を更新し、HTTP クライアント統合などのユースケースをサンプル化。

これにより、pekko 互換の `pipeToSelf` を Rust イディオムで提供しつつ、将来的な ask/message adapter の拡張とも整合する設計が整う。
