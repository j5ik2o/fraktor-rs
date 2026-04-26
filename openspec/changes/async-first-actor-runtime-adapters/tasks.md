## Phase 1: 事前確認と設計固定

- [ ] 1.1 `docs/plan/2026-04-26-mailbox-dispatcher-async-judgement.md` の固定前提を再確認する。
- [ ] 1.2 `TokioExecutor` を task executor に再定義するか、`TokioTaskExecutor` を新設するかを決める。
- [ ] 1.3 Embassy adapter crate 名を `actor-adaptor-embassy` で確定するか確認する。
- [ ] 1.4 `Actor::receive` / `MessageInvoker::invoke` / `Mailbox::run` は本 change で async 化しないことを実装方針として固定する。
- [ ] 1.5 `ActorContext::pipe_to_self` / `pipe_to` と `ContextPipeTask` を future-to-message kernel contract として先に固定し、その後 `TypedActorContext::pipe_to_self` / `pipe_to` を薄い wrapper として整える方針を固定する。

## Phase 2: Tokio executor family の分離

- [ ] 2.1 既存 `TokioExecutor` の tests を読み、`spawn_blocking` 前提になっている箇所を洗い出す。
- [ ] 2.2 default dispatcher 用 executor を Tokio task 実行へ変更または新設する。
- [ ] 2.3 `TokioBlockingExecutor` を追加し、`spawn_blocking` をここへ移す。
- [ ] 2.4 `TokioBlockingExecutorFactory` を追加する。
- [ ] 2.5 `std::dispatch::dispatcher` の public re-export を更新する。
- [ ] 2.6 public surface tests に `TokioBlockingExecutorFactory` を追加する。

## Phase 3: default / blocking dispatcher 登録の分離

- [ ] 3.1 `Dispatchers::ensure_default` / `ensure_default_inline` / `replace_default_inline` の現状挙動を確認する。
- [ ] 3.2 既存 `with_dispatcher_factory` だけで std Tokio helper が default / blocking を別登録できるか確認する。
- [ ] 3.3 既存 API で十分でなければ `with_default_dispatcher_factory` / `with_blocking_dispatcher_factory` 相当を追加する。
- [ ] 3.4 std Tokio 用 config helper を追加し、default に Tokio task executor、blocking に Tokio blocking executor、tick driver に `TokioTickDriver` を設定する。
- [ ] 3.5 `DispatcherSelector::Blocking` が std Tokio helper 構成で blocking executor 側に到達する integration test を追加する。

## Phase 4: untyped kernel future-to-message contract

- [ ] 4.1 `ActorContext::pipe_to_self` / `pipe_to` と `ContextPipeTask` の現在の delivery / wakeup / error 観測経路を確認する。
- [ ] 4.2 `ContextPipeTask` の waker が actor cell の再 poll 経路に戻ることを regression test で固定する。
- [ ] 4.3 actor 停止済みや cell 不在時の `PipeSpawnError` が握りつぶされず caller に返ることを regression test で固定する。
- [ ] 4.4 `pipe_to` の `None` delivery suppression と target delivery failure の観測経路を test / docs で固定する。
- [ ] 4.5 untyped kernel contract を rustdoc に記載し、typed wrapper が依存する前提を明文化する。

## Phase 5: typed thin wrapper

- [ ] 5.1 `TypedActorContext::pipe_to_self` / `pipe_to` が untyped kernel adapter に委譲していることを確認する。
- [ ] 5.2 typed wrapper が `AnyMessage` を caller に露出せず、`AdaptMessage` 経由で typed message に戻していることを確認する。
- [ ] 5.3 typed `pipe_to_self` の Ok / Err future completion が self message として戻る integration test を追加または強化する。
- [ ] 5.4 typed `pipe_to_self` の adapter failure が actor の adapter failure 経路で観測されることを test / docs で固定する。
- [ ] 5.5 `ask` / `ask_with_status` が typed `pipe_to_self` 経路を維持していることを regression test または既存 test で確認する。

## Phase 6: actor API 同期維持と docs

- [ ] 6.1 `TypedActor::receive` と `Behaviors::receive_message` が同期 handler のままであることを public surface test または compile test で固定する。
- [ ] 6.2 actor / behavior handler が `Future` を返す新 contract をこの change で追加しないことを docs / tasks に明記する。
- [ ] 6.3 Pekko `ActorContext.pipeToSelf` と同じく、future completion は message として actor に戻し、state 更新は completion message handler で行うことを rustdoc または cookbook に記載する。
- [ ] 6.4 restart 後の in-flight future completion は通常 message として扱われることを docs / tests で明記する。
- [ ] 6.5 cancel が必要なユースケースは generation token を message に含める方針を cookbook または rustdoc に記載する。

## Phase 7: Embassy adapter crate

- [ ] 7.1 `modules/actor-adaptor-embassy` の Cargo package を追加する。
- [ ] 7.2 `actor-core` へ Embassy 依存が入らないことを確認する。
- [ ] 7.3 `EmbassyExecutor` を追加し、`Executor::execute` では bounded ready queue への enqueue と signal notify のみを行う。
- [ ] 7.4 Embassy task 側で ready queue を drain する driver を追加する。
- [ ] 7.5 ready queue 満杯時は block せず `ExecuteError` を返す。
- [ ] 7.6 `TickDriverKind::Embassy` を追加する。
- [ ] 7.7 `EmbassyTickDriver` を追加し、`embassy-time` で tick を供給する。
- [ ] 7.8 `EmbassyMailboxClock` または同等 helper で mailbox throughput deadline clock を注入する。
- [ ] 7.9 Embassy adapter の compile check 対象 target / feature を scripts または CI 方針に合わせて整理する。

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
