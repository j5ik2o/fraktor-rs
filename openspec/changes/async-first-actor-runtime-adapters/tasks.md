## Phase 1: 最新コード baseline の反映

- [x] 1.1 `docs/plan/2026-04-26-mailbox-dispatcher-async-judgement.md` の固定前提を再確認する。
- [x] 1.2 `TokioExecutor` は既存名のまま default task executor に再定義し、blocking 用は `TokioBlockingExecutor` を新設する方針に固定する。
- [x] 1.3 Embassy adapter crate 名は `actor-adaptor-embassy` に固定する。
- [x] 1.4 `Actor::receive` / `MessageInvoker::invoke` / `Mailbox::run` は本 change で async 化しないことを実装方針として固定する。
- [x] 1.5 現行コードでは `ActorContext::pipe_to_self` / `pipe_to`、`ContextPipeTask`、waker が既に untyped kernel contract として存在することを確認する。
- [x] 1.6 現行コードでは `TypedActorContext::pipe_to_self` / `pipe_to` が untyped kernel adapter に委譲する薄い wrapper であることを確認する。
- [x] 1.7 現行コードでは `TickDriverKind::Embassy` と `modules/actor-adaptor-embassy` が未実装であることを確認する。

## Phase 2: Tokio executor family の分離

- [x] 2.1 既存 `TokioExecutor` が `spawn_blocking` 前提であることを確認する。
- [ ] 2.2 `TokioExecutor` を default dispatcher 用の Tokio task executor に変更する。
- [ ] 2.3 `TokioBlockingExecutor` を追加し、`spawn_blocking` をここへ移す。
- [ ] 2.4 `TokioBlockingExecutorFactory` を追加する。
- [ ] 2.5 `std::dispatch::dispatcher` の public re-export を更新する。
- [ ] 2.6 public surface tests に `TokioBlockingExecutor` / `TokioBlockingExecutorFactory` を追加する。
- [ ] 2.7 `TokioExecutor` tests から default executor が `spawn_blocking` を使う前提を削除する。

## Phase 3: default / blocking dispatcher 登録の分離

- [x] 3.1 `Dispatchers::ensure_default` / `ensure_default_inline` / `replace_default_inline` の現状挙動を確認する。
- [x] 3.2 `ActorSystemConfig::with_dispatcher_factory` で default / blocking reserved id を個別登録できることを確認する。
- [x] 3.3 core に `with_default_dispatcher_factory` / `with_blocking_dispatcher_factory` を追加せず、既存 `with_dispatcher_factory` を使う方針に固定する。
- [ ] 3.4 std Tokio 用 config helper を追加し、default に `TokioExecutorFactory`、blocking に `TokioBlockingExecutorFactory`、tick driver に `TokioTickDriver` を設定する。
- [ ] 3.5 `std_actor_system_config` は mailbox clock helper として維持し、Tokio dispatcher helper と責務を混ぜない。
- [ ] 3.6 `DispatcherSelector::Blocking` が std Tokio helper 構成で blocking executor 側に到達する integration test を追加する。

## Phase 4: untyped kernel future-to-message contract

- [x] 4.1 `ActorContext::pipe_to_self` / `pipe_to` と `ContextPipeTask` の現在の delivery / wakeup / error 観測経路を確認する。
- [x] 4.2 `ContextPipeTask` の waker が actor cell の再 poll 経路に戻ることは既存 regression test で固定済みであることを確認する。
- [x] 4.3 actor cell 不在時の `PipeSpawnError::ActorUnavailable` は既存 tests で固定済みであることを確認する。
- [x] 4.4 `pipe_to` の external target delivery は既存 tests で固定済みであることを確認する。
- [ ] 4.5 actor 停止済み / mailbox closed 後の completion delivery failure が観測可能であることを regression test で固定する。
- [ ] 4.6 `pipe_to` の `None` delivery suppression を test / rustdoc で固定する。
- [ ] 4.7 untyped kernel contract を rustdoc に記載し、typed wrapper が依存する前提を明文化する。

## Phase 5: typed thin wrapper

- [x] 5.1 `TypedActorContext::pipe_to_self` / `pipe_to` が untyped kernel adapter に委譲していることを確認する。
- [x] 5.2 typed wrapper が `AnyMessage` を caller に露出せず、typed message mapper を受け取る signature であることを確認する。
- [x] 5.3 typed `pipe_to` の Ok / Err future completion が external target に届く既存 tests を確認する。
- [x] 5.4 typed `pipe_to` の adapter failure が warn log で観測される既存 tests を確認する。
- [x] 5.5 typed `pipe_to_self` の Ok future completion は existing typed tests / typed user flow で固定済みであることを確認する。
- [x] 5.6 `ask` / `ask_with_status` が typed `pipe_to_self` 経路を使っていることを確認する。
- [ ] 5.7 typed `pipe_to_self` の Err future completion が self message として戻る regression test を追加する。
- [ ] 5.8 typed `pipe_to_self` の adapter failure が actor の adapter failure 経路、dead letter、または warn log で観測されることを test / docs で固定する。

## Phase 6: actor API 同期維持と docs

- [x] 6.1 `TypedActor::receive` と `Behaviors::receive_message` が同期 handler のままであることを確認する。
- [ ] 6.2 actor / behavior handler が `Future` を返す新 contract をこの change で追加しないことを docs / rustdoc に明記する。
- [ ] 6.3 Pekko `ActorContext.pipeToSelf` と同じく、future completion は message として actor に戻し、state 更新は completion message handler で行うことを rustdoc または cookbook に記載する。
- [ ] 6.4 restart 後の in-flight future completion は通常 message として扱われることを docs / tests で明記する。
- [ ] 6.5 cancel が必要なユースケースは generation token を message に含める方針を cookbook または rustdoc に記載する。

## Phase 7: Embassy adapter crate

- [ ] 7.1 `modules/actor-adaptor-embassy` の Cargo package を追加する。
- [x] 7.2 `actor-core` へ Embassy 依存が入っていないことを確認する。
- [x] 7.3 `AutoProfileKind::Embassy` は既に存在するが、`TickDriverKind::Embassy` は未実装であることを確認する。
- [ ] 7.4 `EmbassyExecutor` を追加し、`Executor::execute` では bounded ready queue への enqueue と signal notify のみを行う。
- [ ] 7.5 Embassy task 側で ready queue を drain する driver を追加する。
- [ ] 7.6 ready queue 満杯時は block せず `ExecuteError` を返す。
- [ ] 7.7 `TickDriverKind::Embassy` を追加する。
- [ ] 7.8 `EmbassyTickDriver` を追加し、`embassy-time` で tick を供給する。
- [ ] 7.9 `EmbassyMailboxClock` または同等 helper で mailbox throughput deadline clock を注入する。
- [ ] 7.10 Embassy adapter の compile check 対象 target / feature を scripts または CI 方針に合わせて整理する。

## Phase 8: docs / gap-analysis 更新

- [ ] 8.1 `docs/plan/2026-04-26-mailbox-dispatcher-async-judgement.md` に採用した API 名と実装結果を反映する。
- [ ] 8.2 mailbox gap analysis に `pushTimeOut` 系 blocking mailbox は意図的低優先度であることを追記する。
- [ ] 8.3 std Tokio helper と blocking dispatcher の使い分けを docs または rustdoc に記載する。
- [ ] 8.4 Embassy adapter の制約を docs または crate-level rustdoc に記載する。

## Phase 9: 検証

- [ ] 9.1 `actor-adaptor-std` の targeted tests を実行する。
- [ ] 9.2 `actor-core` untyped `pipe_to_self` / `pipe_to` の targeted tests を実行する。
- [ ] 9.3 `actor-core` typed `pipe_to_self` / `pipe_to` / `ask` の targeted tests を実行する。
- [ ] 9.4 Embassy adapter の compile check / unit contract tests を実行する。
- [ ] 9.5 `rtk rg -n "spawn_blocking" modules/actor-adaptor-std/src/std/dispatch/dispatcher` で default executor に `spawn_blocking` が残っていないことを確認する。
- [ ] 9.6 ソースコード編集後の最終確認として `./scripts/ci-check.sh ai all` を実行し、完了を待つ。
