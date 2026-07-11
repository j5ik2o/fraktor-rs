# RFC 0008: typed 層

| 項目 | 内容 |
|------|------|
| Status | As-built |
| 対象コード | `modules/actor-core-typed/src/` |
| 関連文書 | RFC 0004（untyped の supervision / lifecycle）, `CONTEXT.md`（Typed Actor API Boundary / Typed Receptionist / Receptionist Setup） |
| 最終照合日 | 2026-07-11 |

## 1. 用語

Typed Actor API Boundary (型付きアクター API 境界)、Typed Receptionist、Receptionist Setup。

## 2. 概要

typed 層は untyped kernel の**ラッパー**であり、独自のランタイムを持たない。`Behavior<M>` を `BehaviorRunner`（`TypedActor<M>` 実装）が評価し、それを `TypedActorAdapter<M>`（untyped `Actor` trait 実装）が kernel に接続する。`TypedActorRef<M>` / `TypedActorSystem<M>` / `TypedProps<M>` はそれぞれ untyped 対応物の型付きラッパーである。

## 3. 規範仕様

### 3.1 Behavior と directive（宣言された挙動）

- **TY-1.** `Behavior<M>` は start / message / signal の 3 ハンドラ（いずれも `Result<Behavior<M>, ActorError>` を返す）と `BehaviorDirective` を保持する。ハンドラは `ArcShared` 共有であり `Behavior` は clone 可能。
- **TY-2.** `BehaviorDirective`（6 値）のランタイム効果（`BehaviorRunner::apply_transition`）:
  - `Same` / `Ignore` — 現在の behavior を維持
  - `Active` — 返された behavior に交代
  - `Unhandled` — 現在の behavior を維持し、`EventStreamEvent::UnhandledMessage` を発行しなければならない（MUST）
  - `Empty` — `UnhandledMessage` を発行した上で以後すべてのメッセージを未処理として扱う empty behavior に固定
  - `Stopped` — 初回に `ctx.stop_self()` を呼ぶ。停止処理中でも新しい signal handler 付きの behavior が返れば差し替え、`PostStop` 用のハンドラを失わない
- **TY-3.** DSL ファクトリ `Behaviors` は Pekko `scaladsl` 対応の生成子を提供する（`receive` / `receive_message` / `receive_partial`（None → unhandled）/ `setup` / `supervise` / `with_timers` / `intercept` / `monitor` / `transform_messages` / `narrow` 等）。

### 3.2 untyped への接続（宣言された挙動）

- **TY-4.** `TypedActorAdapter<M>` の `receive` は adapter 系メッセージを先に処理した後、`AnyMessage::downcast_ref::<M>()` を試み、失敗時は `ActorError::recoverable("typed actor received unexpected message")` を返す。これは通常の失敗として supervisor 判定（RFC 0004）へ流れ、専用イベントは発行されない。
- **TY-5.** untyped lifecycle フックと `BehaviorSignal`（6 値）の対応は固定である:
  - `post_stop` → `PostStop`（先に stopping フラグを立てる）
  - `pre_restart` → `PreRestart`、`post_restart` → `PostRestart`（adapter は PostRestart 伝達後に `pre_start` を再実行し、restart 後の behavior を再構築する）
  - `on_terminated(pid)` → `Terminated`
  - `on_child_failed` → `ChildFailed`、adapter 失敗 → `MessageAdaptionFailure`
- **TY-6.** **DeathPactError**: watch した相手の `Terminated` シグナルを処理しなかった場合（signal handler が未登録、または `Unhandled` を返した場合）、runner は `DeathPactError` を含む `ActorError::recoverable` を返さなければならない（MUST）。すなわち「watch したら Terminated を処理する、さもなくば自分が失敗する」という Pekko の death pact 契約が成立する。`MessageAdaptionFailure` の未処理も同様に失敗へ倒れる。
- **TY-7.** supervise DSL: `Behaviors::supervise(b).on_failure(strategy)` は behavior に `SupervisorStrategyConfig`（`Standard` / `Backoff`）を載せ、`BehaviorRunner::supervisor_strategy` が untyped の `Actor::supervisor_strategy` として返す。`on_failure_of::<E>` はエラー型別ハンドラを合成 decider として積む。

### 3.3 TypedActorSystem（宣言された挙動）

- **TY-8.** 生成 API は `create_from_props` / `create_with_noop_guardian` / `create_from_props_with_init` / `create_from_behavior_factory`。guardian behavior は `GuardianStartupActor` でラップされ、system receptionist のインストール → 呼び出し側 init → user guardian 開始の順で起動する。
- **TY-9.** ほぼ全メソッドは untyped `ActorSystem` へ委譲であり、`user_guardian_ref()` は untyped ref の型付きラップである。

### 3.4 Receptionist / PubSub / SpawnProtocol / Delivery（宣言された挙動）

- **TY-10.** Receptionist: `ServiceKey<M>`（id + `TypeId`）単位で `Register` / `Deregister` / `Subscribe` / `Unsubscribe` / `Find` を処理する。Register 時は対象を watch して登録し（同一 ref の再登録は無視）、購読者へ `Listing` を通知する。Subscribe 時はまず現在の一覧を即時送付してから購読登録する。`Terminated` 観測時は全キーから除去して影響キーの購読者へ再通知する。ack（`Registered`）送信の失敗は警告ログのみで登録自体は成功扱い（ベストエフォート）。
- **TY-11.** Receptionist は `"receptionist"` という名前の system top-level actor として bootstrap 時にインストールされる。未インストール状態での `receptionist()` は panic する。
- **TY-12.** PubSub `Topic`: `TopicCommand` はファクトリ経由でのみ構築できる（直接構築はコンパイル不可として doctest で宣言）。配送は排他的 2 経路——`topic_instances`（他インスタンス）が空ならローカル購読者へ直接、非空なら各インスタンスへ `MessagePublished` を転送し受信側がローカル配送する（重複配送しない設計）。初回購読で自身を receptionist に登録し、購読者ゼロで deregister する。
- **TY-13.** SpawnProtocol: `spawn` / `spawn_anonymous` を処理するコマンド actor。spawn 失敗は警告ログのみで actor は継続し、要求元の ask はタイムアウトまで pending のままになる（暗黙の挙動）→ OQ-TY-2。
- **TY-14.** Delivery（reliable delivery）: `ProducerController` / `ConsumerController` は seq_nr ベースの **at-least-once + 再送** 契約を持つ。consumer は `confirmed_seq_nr` 以下を重複として破棄し、欠番を検出すると `Resend{from}` を要求する。`WorkPullingProducerController` は receptionist でワーカーを動的発見する。`DurableProducerQueue` は確認済み/未確認メッセージの不変スナップショットを永続化し、クラッシュ後の再送を支える。本 RFC では概説に留め、詳細仕様は将来の個別 RFC とする。

## 4. 状態機械

- **BehaviorRunner**: 「現在の behavior + stopping フラグ」が状態であり、directive（TY-2）が遷移を定める。`Empty` は吸収状態（メッセージについて）、`Stopped` は stop_self を一度だけ発火する。
- **ConsumerController の seq_nr 追跡**: `confirmed_seq_nr` 単調増加、ギャップで resend 要求（TY-14）。

## 5. 不変条件

- **INV-TY-1**: typed actor に届いた `M` 以外の user メッセージは、黙って握りつぶされることなく失敗（supervisor 判定）として現れる（TY-4）。
- **INV-TY-2**: `Unhandled` / `Empty` による未処理は必ず `UnhandledMessage` イベントとして観測可能である（TY-2）。
- **INV-TY-3**: watch した Terminated の未処理は必ず失敗（DeathPactError）に変換される（TY-6）。
- **INV-TY-4**: Receptionist の Listing は「終了した ref を含み続けない」（Terminated 観測での除去 + 再通知により成立）。
- **INV-TY-5**: Topic の 1 publish が同一購読者へ二重配送されることはない（排他的 2 経路、TY-12）。

## 6. 機械的な問いへの回答

- **エラー時の倒れ先は?** — ダウンキャスト失敗・death pact・adapter 失敗はすべて `ActorError` として supervision へ（fail-closed）。receptionist ack や Topic 配送の失敗はベストエフォート（fail-open、ログのみ）。
- **空/未設定のとき?** — signal handler 未登録で Terminated を受けると DeathPactError（TY-6）。receptionist 未インストールの参照は panic（TY-11）。
- **同時に 2 つ来たら?** — typed 層は独自の並行性を持たず、すべて untyped mailbox の逐次性（RFC 0002）に還元される。

## 7. Open Questions

| # | 観測した事実 | 質問 | 影響 |
|---|-------------|------|------|
| OQ-TY-1 | `TypedAskResponse::from_untyped` の型対応は debug assert のみで、実行時は値取得時に `TypedAskError::TypeMismatch` として検出される | reply 型の不一致を送信時点で拒否する強い検査は必要か | ask の型安全性の境界 |
| OQ-TY-2 | SpawnProtocol の spawn 失敗時、要求元 ask はタイムアウトまで待たされる（TY-13） | 失敗応答（Status 等）を返すべきか | spawn 失敗の観測遅延 |

形式化候補（Lean）: TY-6 の death pact（「watch ⇒ Terminated 処理 or 失敗」）は、RFC 0005 INV-DW-1（exactly-once 通知）と合成すると「watch した終了は必ず観測または失敗に至る」という端到端の定理になる。ConsumerController の seq_nr プロトコル（重複破棄・欠番再送）は at-least-once 配送の古典的な検証対象。

## 8. 参照

- Pekko: `actor-typed`（`Behaviors` / `MessageAndSignals` / `Receptionist` / delivery パッケージ）
- RFC 0004（supervision との接続）、RFC 0005（DeathWatch）、RFC 0007（UnhandledMessage / AdapterFailure の発行）
