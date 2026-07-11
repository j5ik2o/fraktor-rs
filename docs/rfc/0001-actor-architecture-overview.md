# RFC 0001: actor アーキテクチャ概観

| 項目 | 内容 |
|------|------|
| Status | As-built |
| 対象コード | `modules/actor-core-kernel` / `modules/actor-core-typed` / `modules/actor-adaptor-std` / `modules/actor-adaptor-embassy` |
| 関連文書 | ADR 0002, ADR 0003, `docs/gap-analysis/actor-gap-analysis.md`, `.kiro/steering/structure.md`, `lints/port-adaptor-boundary-lint/SPEC.md` |
| 最終照合日 | 2026-07-11 |

## 1. 用語

本 RFC は `CONTEXT.md` の canonical term を用いる。特に Actor Cell (アクターセル)、Actor System State (アクターシステム状態)、Kernel Public Surface。

## 2. 概要

actor ドメインは 4 クレートで構成される。

| クレート | 層 | 責務 |
|---------|----|------|
| `actor-core-kernel` | core (`no_std`) | untyped actor ランタイムの中核。actor / dispatch / event / pattern / routing / serialization / support / system の 8 公開モジュール（`modules/actor-core-kernel/src/lib.rs`） |
| `actor-core-typed` | core (`no_std`) | untyped kernel を包む typed API（`Behavior<M>` / `TypedActorRef<M>` / receptionist / pub-sub / delivery） |
| `actor-adaptor-std` | adaptor (std) | kernel が定義する port の std / Tokio 実装 |
| `actor-adaptor-embassy` | adaptor (`no_std` embedded) | 同 port の Embassy 実装 |

依存方向は core → adaptor の一方向ではなく、**core が port（trait）を定義し、adaptor がそれを実装する**（依存性逆転）。adaptor は core の型を参照するが、core は adaptor を知らない。

```
actor-core-typed ──────► actor-core-kernel（port 定義）
                              ▲            ▲
                 実装 ────────┘            └──────── 実装
        actor-adaptor-std                actor-adaptor-embassy
```

## 3. 規範仕様

### 3.1 宣言された挙動

- **A-1.** `actor-core-kernel` と `actor-core-typed` は `#![cfg_attr(not(test), no_std)]` であり、`std` へ直接依存してはならない（MUST NOT）。`alloc` のみ使用できる。機械的強制: `#![deny(cfg_std_forbid)]`（各 `lib.rs`）。
- **A-2.** ホスト環境（スレッド・時刻・Tokio・panic 捕捉）への接続は、kernel が定義する port trait（`TickDriver` / `Executor` / `ExecutorFactory` / `Blocker` / `Clock` / `InvokeGuard` / `EventStreamSubscriber` / `LoggerWriter` 等。詳細は RFC 0009）を adaptor 側で実装する形で行わなければならない（MUST）。
- **A-3.** adaptor の公開型は core の concrete API facade を保持してはならない（MUST NOT）。core 定義の port trait を実装することは許可される。機械的強制: `port-adaptor-boundary-lint`（`lints/port-adaptor-boundary-lint/SPEC.md`）。
- **A-4.** 公開 API はすべて rustdoc を持たなければならない（MUST）。機械的強制: `#![deny(missing_docs)]`（各 `lib.rs`）。
- **A-5.** 参照実装は Apache Pekko（`references/pekko`）と protoactor-go（`references/protoactor-go`）である。Pekko とセマンティクスを揃える箇所には、実装コメントで対応する Pekko ソース位置を明記する（SHOULD）。例: `dispatch/mailbox/base.rs` の `Mailbox.scala` 行番号参照、`actor/children_container.rs` の `ChildrenContainer.scala` 対応表。

### 3.2 暗黙の挙動

- **A-6.** ランタイムの実行モデルは「外部から供給される tick とメッセージ配送」で駆動される。kernel 自身はスレッドも時刻も持たない。`ActorSystemConfig` は tick driver を必須とし、欠落時はシステム構築が `SpawnError::SystemBuildError("tick driver is required")` で失敗する（`system/state/system_state.rs`）。
- **A-7.** 可変状態の共有は `SharedLock` / `SharedRwLock`（`with_read` / `with_write`）に限定され、ロックガードを外部へ返さない。ロジック本体は `&mut self` で設計される（`.agents/rules/rust/immutability-policy.md` の方針がコードベース全域に適用されている）。
- **A-8.** guardian 階層は Root (`/`) → System (`/system`) → User (`/user`) の 3 段で固定であり、`ActorSystem::actor_of` で生成されるトップレベル actor はすべて user guardian の子になる（`system/guardian/`、詳細は RFC 0005）。

## 4. 構成要素の対応表

| 概念 | 主な型 | 定義位置 | 詳細 RFC |
|------|--------|---------|----------|
| システム | `ActorSystem` | `modules/actor-core-kernel/src/system/base.rs` | 0005 |
| 実行コンテナ | `ActorCell` | `modules/actor-core-kernel/src/actor/actor_cell.rs` | 0004 |
| 送信ハンドル | `ActorRef` / `ChildRef` | `modules/actor-core-kernel/src/actor/actor_ref/base.rs` | 0003 |
| メッセージ | `AnyMessage` / `Envelope` / `SystemMessage` | `modules/actor-core-kernel/src/actor/messaging/` | 0002, 0004 |
| mailbox | `Mailbox` / `MessageQueue` 実装群 | `modules/actor-core-kernel/src/dispatch/mailbox/` | 0002 |
| dispatcher | `MessageDispatcher` 実装 3 種 | `modules/actor-core-kernel/src/dispatch/dispatcher/` | 0003 |
| 監督 | `SupervisorStrategy` / `ChildrenContainer` | `modules/actor-core-kernel/src/actor/supervision/`, `actor/children_container.rs` | 0004 |
| 死亡監視 | DeathWatch (`WatchKind` / `DeathWatchNotification`) | `modules/actor-core-kernel/src/actor/actor_cell_death_watch.rs` | 0005 |
| スケジューラ | `Scheduler` / `TickDriver` | `modules/actor-core-kernel/src/actor/scheduler/` | 0006 |
| 可観測性 | `EventStream` / Dead Letter | `modules/actor-core-kernel/src/event/`, `actor/actor_ref/dead_letter/` | 0007 |
| typed 層 | `Behavior<M>` ほか | `modules/actor-core-typed/src/` | 0008 |
| routing / serialization / patterns | `Router` / `SerializationRegistry` / `CircuitBreaker` ほか | `modules/actor-core-kernel/src/routing/`, `modules/actor-core-kernel/src/serialization/`, `modules/actor-core-kernel/src/pattern/` | 0010 |

公開面の規模（2026-07-11 時点の `rg '^\s*pub (struct|enum|trait|type)\b'` による概数）: kernel = struct 257 / enum 80 / trait 46 / type 4、typed = struct 78 / enum 10 / trait 8 / type 5、adaptor-std = struct 20、adaptor-embassy = struct 4。adaptor 2 クレートに公開 trait / enum が存在しないことは、「型の定義は core、実装だけが adaptor」という A-2 / A-3 の帰結である。

## 5. 不変条件

- **INV-ARCH-1**: `*-core` クレートは std のシンボルへ直接依存しない（`cfg_std_forbid` により機械的に成立）。
- **INV-ARCH-2**: adaptor クレートは port trait の実装のみを公開し、新しいドメイン契約（trait / enum）を定義しない（`port-adaptor-boundary-lint` と公開面カウントにより成立）。
- **INV-ARCH-3**: kernel はスレッド・タイマー・panic 捕捉を自前で生成せず、すべて port 経由で受け取る（`TickDriver` / `Executor` / `InvokeGuard` の設計により成立。既定の `InvokeGuard` は素通しの `NoopInvokeGuard` であり、panic 捕捉は adaptor-std の `PanicInvokeGuard` が担う）。

## 6. 機械的な問いへの回答

- **空/未設定のとき何が起きる?** — tick driver 未設定はシステム構築エラー（A-6）。dispatcher / mailbox は既定実装（`fraktor.actor.default-dispatcher` / `fraktor.actor.default-mailbox`）に倒れる（RFC 0002 / 0003）。
- **この値は誰が決める?** — 実行環境依存の値（スレッド数、tick の実時間間隔、panic の扱い）はすべて adaptor 側の port 実装が決め、kernel は論理契約（tick 数、throughput 件数）だけを決める。

## 7. Open Questions

| # | 観測した事実 | 質問 | 影響 |
|---|-------------|------|------|
| OQ-ARCH-1 | 既定 dispatcher の executor は `InlineExecutor`（呼び出しスレッドで同期実行）であり、rustdoc には「deterministic tests 用」と書かれている（`dispatch/dispatcher/inline_executor.rs`, `dispatchers.rs`） | 既定構成が inline 実行であることは意図した既定値か、それとも std adaptor の executor を既定にすべき過渡状態か | 既定構成の並行性の期待値（actor が本当に並行に走るか）が変わる |
| OQ-ARCH-2 | `actor-adaptor-embassy` には `Blocker` / `Clock` port の実装がない | embedded では kernel 内蔵の `SpinBlocker` 等で足りるという意図か、未実装ギャップか | embedded での同期待ち・時刻依存機能（circuit breaker 等）の可用性 |

形式化候補（Lean）: 本 RFC 自体には状態機械がないため対象なし。各サブシステム RFC の候補を参照。

## 8. 参照

- ADR 0002 / 0003（`docs/adr/`）
- `docs/gap-analysis/actor-gap-analysis.md`, `docs/gap-analysis/actor-mailbox-gap-analysis.md`
- Pekko: `references/pekko`
