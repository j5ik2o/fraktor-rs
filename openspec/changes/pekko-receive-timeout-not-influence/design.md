## Context

fraktor-rs の `actor_cell.rs:1527` は、user message の `invoke_user` が成功した直後に **無条件で** `ctx.reschedule_receive_timeout()` を呼び、現在の timeout 状態を破棄して再スケジュールする。Pekko `dungeon/ReceiveTimeout.scala:40-42,71-76` は、`NotInfluenceReceiveTimeout` marker trait を実装するメッセージの場合にこの reset をスキップする契約を持ち、特に periodic / 内部由来のメッセージが idle 検知を壊さないようにしている (`Actor.scala:165` の trait 定義 + `Actor.scala:81` で `Identify` が mix-in)。fraktor-rs 側にはこの抑制機構が一切無く、ユーザーが「このメッセージは timeout に影響させない」と宣言する手段も無い。

`AnyMessage` は現状 `payload: ArcShared<dyn Any + Send + Sync>` + `sender` + `is_control: bool` の 3 要素。`is_control` と並列に「本メッセージは receive timeout に影響しない」フラグを 1 本追加することで、`ActorCellInvoker::invoke` がマーカーチェックで分岐できるようになる。

fraktor-rs の `ReceiveTimeout` 構造体 (`messaging/receive_timeout.rs:8`) は Pekko の `ReceiveTimeout` case object と対照的に **自動送信されない**。ユーザーが `ActorContext::set_receive_timeout(duration, message: AnyMessage)` に渡した `message` がそのまま timer 経由で自身に届く仕様。これは本 change のスコープを「ユーザーが NotInfluence を表明する手段」と「内部で `Identify` を送る箇所でマーカーを立てる」の 2 点に絞る根拠になる。

## Goals / Non-Goals

**Goals:**

- `NotInfluenceReceiveTimeout` marker trait を導入し、任意のユーザーメッセージ型がこれを `impl` できるようにする (Pekko `Actor.scala:165` 対応)。
- `AnyMessage` に `not_influence_receive_timeout: bool` フラグを追加し、`AnyMessage::not_influence::<T: NotInfluenceReceiveTimeout>(payload)` コンストラクタでフラグを立てる (Pekko `isInstanceOf[NotInfluenceReceiveTimeout]` チェックの Rust 置換)。
- `ActorCellInvoker::invoke` の user message 成功ブランチに「marker あり → reschedule skip」のガードを追加。
- 内部で `Identify` メッセージを封筒化する箇所 (`actor_selection/selection.rs:77`) を `AnyMessage::not_influence` 経由に書き換え、Pekko `Identify extends AutoReceivedMessage with NotInfluenceReceiveTimeout` に準拠させる。
- 契約を pin する kernel テスト 6 件追加 (marker 付き skip / marker 無し reset / Identify internal path / Clone 伝播 / View getter / 既存 failure regression の維持。詳細は Decision 5)。
- `docs/gap-analysis/actor-gap-analysis.md` AC-M5 を done 化、medium カウント 11 → 10。

**Non-Goals:**

- Pekko `Timers` / `ClassicTimerScheduler` (fraktor-rs `classic_timer_scheduler.rs`) の timer メッセージへの `NotInfluenceReceiveTimeout` 自動付与 (Pekko `dungeon/TimerSchedulerImpl.scala:37-40`)。Pekko では timer 内部メッセージ `NotInfluenceReceiveTimeoutTimerMsg` が自動で marker を持つが、本 change では触らない。後続 change でのみ対応する。
- Pekko `AutoReceivedMessage` ヒエラルキーの全面移植 (fraktor-rs は個別 downcast で同等機能を実現しており、本 change のスコープ外)。
- `receive_timeout_state.rs` は本 change 内で `schedule_generation: u64` field 追加 (テスト + 診断用途) に限定する。タイマー構造のリファクタ / scheduler ↔ mailbox 経路のリファクタはスコープ外。本 change の主軸は `AnyMessage` のフラグ伝播と `invoke` 側ガード。
- Pekko `ReceiveTimeout` case object の「fraktor-rs `ReceiveTimeout` 構造体を auto-message として timer が自動送出する」方向の改修 (現行は user 提供 message で十分動作しており、behavior 差は本 change のターゲットではない)。
- MB-M2/M3, AC-M1/2/3/4, ES-M1, FS-M1/2, AL-M1 (別 change)。

## Decisions

### Decision 1: `NotInfluenceReceiveTimeout` を marker trait として導入する (方式 A)

候補:
- **A (採用)**: `pub trait NotInfluenceReceiveTimeout: Any + Send + Sync {}` を定義し、`Identify` とユーザー型がそれぞれ `impl` する (`ReceiveTimeout` 構造体には付けない、Open Questions 参照)。`AnyMessage::not_influence::<T: NotInfluenceReceiveTimeout>(payload)` で型レベルで flag を立てる。
- **B**: `AnyMessage` にフラグだけ追加し、`new_with_not_influence(payload, flag)` のような API にする (trait は無し)。
- **C**: `TypeId` の global registry を持ち、`std::sync::OnceLock<HashSet<TypeId>>` に登録する。

A を採用する理由:
- Pekko の `NotInfluenceReceiveTimeout extends PossiblyHarmful` パターンと型レベルで対応する。Rust 側も trait 1 本で「このメッセージ型は receive timeout に影響しない」という意味論を静的に表現できる。
- ユーザーが marker をつけ忘れる経路が狭い (`AnyMessage::not_influence` は型パラメータの trait bound でコンパイル時に強制可能)。
- `impl NotInfluenceReceiveTimeout for MyMsg {}` だけで declarative に宣言でき、可読性も高い。
- B は型安全性が弱く、どの enqueue 経路でも flag を忘れられる。C は global state が増え、no_std / 複数 ActorSystem の構成下で管理しにくい。

トレードオフ:
- `dyn Any` から `dyn NotInfluenceReceiveTimeout` への downcast は Rust 標準では出来ないため、ランタイムで payload の型を `NotInfluenceReceiveTimeout` として扱うには、**AnyMessage 構築時に型パラメータで判定して flag に畳み込む** 方式しか取れない。これは B と等価な内部表現になるが、trait bound で「封筒化する側が意図して呼ぶ」ことを強制できる点が B より優位。

### Decision 2: `AnyMessage::new(...)` の既存 API は非破壊で維持する

`AnyMessage::new<T>(payload)` は `not_influence_receive_timeout: false` を設定するだけで、シグネチャと従来挙動を維持する。追加の構築経路として `AnyMessage::not_influence::<T: NotInfluenceReceiveTimeout>(payload)` を公開する。

候補:
- **採用**: `new` は従来通り、`not_influence` を追加。
- **却下**: `AnyMessage::new<T>(payload)` 自体で `T: NotInfluenceReceiveTimeout` を自動検出 (negative trait bound / specialization 経由)。Rust 安定版では不可能。
- **却下**: 既存の `AnyMessage::new` を private にして全経路を `new_regular` / `not_influence` に分岐させる。callers が多く diff が大きい割に、得られる value が小さい。

CLAUDE.md「後方互換は不要」でも、「不必要な破壊」は避ける。`AnyMessage::new` は modules 全体で広範に使われており、挙動が変わらないなら API も変えない方が回帰リスクが低い。

### Decision 3: Pekko `cancelReceiveTimeoutIfNeeded` 相当は **導入しない** (fraktor-rs は reset 側だけで十分)

Pekko は `invoke` 入口で `cancelReceiveTimeoutIfNeeded(msg)`、出口で `checkReceiveTimeoutIfNeeded(msg, before)` の 2 箇所で marker を見る。一方 fraktor-rs の `ActorCellInvoker::invoke` は入口で timer をキャンセルしていない (`reschedule_receive_timeout` が cancel + schedule を連続して 1 呼び出しで実行するため)。入口 cancel が無い以上、入口側にガードを入れる必要も無い。

- Pekko 側の入口 cancel は「メッセージ処理中に timer が fire して race で二重配信する」のを避ける設計。
- fraktor-rs の `scheduler_shared` + `reschedule_receive_timeout` は busy 状態を気にせず毎回 cancel + schedule するため、race を起こさない (mailbox は 1 actor 1 スレッドで処理される前提)。
- 本 change は **出口側 (`invoke_user` 成功後) の marker ガード** のみ追加する。これで Pekko `checkReceiveTimeoutIfNeeded` の契約を等価に満たす。

`AnyMessage::is_not_influence_receive_timeout()` は Phase 3.4 で `&self` の公開 getter として用意する。将来、入口 cancel を導入するときは同じ marker 判定をもう一度この getter で呼ぶだけで済む。

### Decision 4: 内部 `Identify` 封筒化の修正範囲

`actor_selection/selection.rs:77` が `AnyMessage::new(Identify::new(...))` で Identify を包んでいる。これを `AnyMessage::not_influence(Identify::new(...))` に変更する。

`Identify` 構造体 (`messaging/identify.rs:10`) 自体にも `impl NotInfluenceReceiveTimeout for Identify {}` を付け、全 caller が `AnyMessage::not_influence` 経由で渡すことを trait bound で強制する。

内部の `ActorCellInvoker::invoke` (`actor_cell.rs:1513-1522`) は Identify を受けた時点で `sender.try_tell(AnyMessage::new(identity))` で `ActorIdentity` を返すが、ここは **Identify 受信側の内部応答**であり、ActorIdentity 側は `NotInfluenceReceiveTimeout` にしない (Pekko 側でも `ActorIdentity` は non-marker)。

### Decision 5: テストは kernel 単体テスト 6 件 + schedule_generation counter を採用

`modules/actor-core/src/core/kernel/actor/actor_cell/tests.rs` と `modules/actor-core/src/core/kernel/actor/messaging/any_message/tests.rs` に以下を追加:

1. `not_influence_message_skips_reschedule`: ユーザー定義型 `NonInfluencingTick` に `impl NotInfluenceReceiveTimeout` を付け、`AnyMessage::not_influence(NonInfluencingTick)` で封筒化。`set_receive_timeout(..)` で timer を走らせ、invoke 前後の `schedule_generation` が同一であることを検証。
2. `regular_message_reschedules_receive_timeout`: `AnyMessage::new(NonInfluencingTick)` で送ると `schedule_generation` が +1 される (回帰テスト)。
3. `identify_message_is_not_influence_by_internal_path`: `actor_selection/selection.rs` の Identify 封筒化を `pub(crate) fn build_identify_envelope(...)` に切り出して helper を直接テスト。`AnyMessage::is_not_influence_receive_timeout() == true` を検証。
4. `not_influence_flag_is_preserved_on_clone`: `AnyMessage::not_influence(NonInfluencingTick).clone()` が flag を保持。
5. `view_exposes_not_influence_flag`: `AnyMessage::not_influence(...).as_view().not_influence_receive_timeout() == true`。
6. `user_message_failure_does_not_reschedule_receive_timeout` (既存) が引き続き pass (regression guard)。

**handle identity 比較ではなく schedule_generation counter を採用する理由**:

- Pekko の `ReceiveTimeoutData` は `(Duration, message)` のペアで handle 管理しており、Rust 側で handle identity を比較するには `ReceiveTimeoutState` の内部構造を露出する必要がある。
- counter 方式は `ReceiveTimeoutState` に `schedule_generation: u64` field を 1 本追加するだけで、schedule 呼び出しの発生回数を数値として比較可能。テストコードも `assert_eq!(before, after)` / `assert_eq!(before + 1, after)` で直感的に書ける。
- production diagnostics としても「timeout の reschedule 頻度」を観測する用途に再利用できる。
- `pub(crate)` で kernel 内に限定すれば public API は汚れない。tasks 8 に具体実装手順を列挙してある。

## Risks / Trade-offs

- **[Risk] `AnyMessage::not_influence` を忘れる**: ユーザーは marker trait を `impl` してもなお `AnyMessage::new(NonInfluencingTick)` で封筒化し得る。この場合 flag は立たず、timeout が reset される。
  → **Mitigation**: rustdoc + openspec spec で「NotInfluenceReceiveTimeout を尊重させたければ `AnyMessage::not_influence` を経由する」ことを明記。長期的には typed API (`TypedActorRef` 経由) で自動判定する拡張を検討 (別 change)。
- **[Risk] Internal Identify 経路の取り漏らし**: 調査時点では封筒化箇所は `actor_selection/selection.rs:77` の 1 件のみだが、将来別経路が追加された場合に marker が漏れる恐れがある。
  → **Mitigation**: `grep -rn "AnyMessage::new(Identify" modules/` を Phase 9 の grep gate (tasks 9.1) で実行、0 件であることを確認する。将来の追加経路もこの gate で検出可能。
- **[Trade-off] `AnyMessage::not_influence` は trait bound で型パラメータを縛るため、消費側 (`invoke`) は trait object を持たない**: 結果として flag は `bool` として畳み込まれ、ランタイムでは trait identity が失われる。Pekko の `isInstanceOf[NotInfluenceReceiveTimeout]` とは意味論的に等価だが、動的型識別は出来ない。将来「特定の marker trait を実装しているかだけを知りたい別用途」が出てきたら、都度 `bool` フラグを追加する方式になる (Pekko でも marker 単位で `isInstanceOf` を書くので同じ)。
- **[Risk] `AnyMessage::clone()` でフラグが正しく伝播しないと regression**: `Clone` 実装は現在 `is_control` を伝播しているが、`not_influence_receive_timeout` も同様に伝播させる必要がある。
  → **Mitigation**: Phase 7 (tasks 7.5) で kernel テストに「clone 後 flag 保持」を明示的に追加 (`not_influence_flag_is_preserved_on_clone`)。

## Migration Plan

本 change は public API に以下の変更を導入する:

1. (additive) `NotInfluenceReceiveTimeout` marker trait の新設。
2. (additive) `AnyMessage::not_influence::<T: NotInfluenceReceiveTimeout>(payload)` コンストラクタ、`AnyMessage::is_not_influence_receive_timeout`、`AnyMessageView::not_influence_receive_timeout`、`AnyMessageView::with_flags` の追加。
3. (**BREAKING**) `AnyMessage::from_parts` / `AnyMessage::into_parts` / `AnyMessage::from_erased` の tuple 要素数拡張。`AnyMessage` 内部 field に `not_influence_receive_timeout: bool` を追加した結果として、`(payload, sender, is_control)` の 3-tuple が `(payload, sender, is_control, not_influence_receive_timeout)` の 4-tuple になる。

- CLAUDE.md「後方互換不要」により破壊的変更許容。
- `AnyMessage::from_parts` / `AnyMessage::into_parts` / `AnyMessage::from_erased` の callers を grep で全特定し、本 change 内で一括修正。
- Rollback: 単一 change として完結しているため、revert 即ロールバック可能。

## Open Questions

なし。以下は調査フェーズで **解決済** の論点なので、実装時の参照事実として残す:

- `AnyMessage::from_parts` / `AnyMessage::into_parts` / `AnyMessage::from_erased` callers が網羅可能 (grep ベースで機械的検出)。
- fraktor-rs の `ReceiveTimeout` 構造体は timer 経由の auto-message ではないため、本 change 対象外。
- Pekko の `ReceiveTimeout` case object 自体は `NotInfluenceReceiveTimeout` を **implement しない** (Pekko `Actor.scala:154` vs `Actor.scala:165`)。fraktor-rs の `ReceiveTimeout` struct にも `impl NotInfluenceReceiveTimeout` を付けない方針で Pekko 準拠を保つ (`set_receive_timeout(duration, AnyMessage::new(ReceiveTimeout))` した場合、timeout は reset されて再発火する — Pekko と同じ挙動)。

ユーザーが「ReceiveTimeout 到着時に self-reset しない」挙動を望む場合は、ユーザー側で `cancel_receive_timeout()` を呼ぶか、`AnyMessage::not_influence(ReceiveTimeoutTick)` + 自前の型 + `impl NotInfluenceReceiveTimeout` で表現する。これが Pekko との互換性と fraktor-rs の API 簡潔さを両立する折衷点。
