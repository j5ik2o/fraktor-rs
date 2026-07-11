# RFC 0009: port-adaptor 契約

| 項目 | 内容 |
|------|------|
| Status | As-built |
| 対象コード | `modules/actor-core-kernel`（port 定義）, `modules/actor-adaptor-std/src/`, `modules/actor-adaptor-embassy/src/`, `lints/port-adaptor-boundary-lint/SPEC.md` |
| 関連文書 | RFC 0001（依存方向）, RFC 0003（Executor）, RFC 0006（TickDriver） |
| 最終照合日 | 2026-07-11 |

## 1. 用語

port（kernel が定義するホスト環境接続用 trait）、adaptor（port の環境別実装クレート）。

## 2. 概要

kernel は実行環境（スレッド・時刻・非同期ランタイム・panic 機構）を port trait として抽象化し、`actor-adaptor-std`（std / Tokio）と `actor-adaptor-embassy`（Embassy）が実装する。境界は dylint（`port-adaptor-boundary-lint`）で機械的に強制される。

## 3. 規範仕様

### 3.1 境界規則（宣言された挙動）

- **PORT-1.** adaptor クレートの公開 struct は、core の concrete API facade を保持してはならず（MUST NOT）、alias 経由で core 型を同名 wrapper として保持してもならない（MUST NOT）。core 定義の port trait の実装、および core の value / error / config / port trait 型をシグネチャで使うことは許可される（`lints/port-adaptor-boundary-lint/SPEC.md` の全文をここに規範化）。

### 3.2 port 一覧

| port（kernel 定義） | 責務 | std 実装 | embassy 実装 |
|--------------------|------|----------|--------------|
| `TickDriver` | tick 供給と executor 駆動 | `StdTickDriver` / `TokioTickDriver` / `TestTickDriver` | `EmbassyTickDriver` |
| `Executor` / `ExecutorFactory` | タスク実行 | `ThreadedExecutor` / `PinnedExecutor` / `AffinityExecutor` / `TokioExecutor` / `TokioTaskExecutor`（+ 各 Factory） | `EmbassyExecutor<N>` / `EmbassyExecutorFactory<N>` |
| `Blocker` | 同期ブロッキング待機 | `StdBlocker` | **なし**（kernel 内蔵 `SpinBlocker` に依存）→ OQ-PORT-1 |
| `Clock`（circuit breaker 用） | 単調時刻 | `StdClock` | **なし** → OQ-PORT-1 |
| `InvokeGuard` / `InvokeGuardFactory` | ハンドラ実行の保護 | `PanicInvokeGuard`（catch_unwind） | なし（既定 `NoopInvokeGuard`） |
| `EventStreamSubscriber` / `LoggerWriter` | 観測の出口 | `DeadLetterLogSubscriber` / `TracingLoggerSubscriber` | なし |
| `MailboxFactory` / `ActorRefProvider` / `RemoteWatchHook` / `RemoteDeploymentHook` | mailbox 生成 / ref 解決 / remote 拡張 | 実装なし（kernel 内で完結、remote は remote 系クレートが実装） | なし |
| `Extension` / `ExtensionId` | プラガブル拡張 | `CircuitBreakersRegistry` | なし |

### 3.3 std adaptor（宣言された挙動）

- **PORT-2.** `StdBlocker` は `Mutex + Condvar` で `block_until(condition)` を実装し、poll 間隔の下限は 1ms にクランプされる（tight spin 防止。rustdoc に「レイテンシと効率のバランス」として宣言）。
- **PORT-3.** tick driver 3 種は共通の下限則 `exec_interval = (resolution / 10).max(1ms)` を持つ。
  - `StdTickDriver`: tick スレッドと executor 駆動スレッドの 2 本を立てる。既定 resolution 10ms。shutdown はフラグ + join
  - `TokioTickDriver`: `Handle::try_current()` 必須（なければ `HandleUnavailable`）。**current-thread ランタイムは `UnsupportedExecutor` として拒否**（MUST）。tick は `tokio::time::interval`（`MissedTickBehavior::Delay`）。停止は abort 後、専用スレッドから block_on で完了待ち（ランタイム内 block_on の panic 回避）
  - `TestTickDriver`: `TickDriverKind::Manual` を返し、runner API の自動有効化（RFC 0006 SCH-8）を引き起こす
- **PORT-4.** executor 5 種のスレッドモデル:
  - `ThreadedExecutor` — タスクごとに新規スレッド（ブロッキング作業向け）。shutdown は no-op
  - `PinnedExecutor` — 専用ワーカー 1 本 + channel。高々 1 タスク in-flight。Drop でも shutdown
  - `AffinityExecutor` — `parallelism` 本のワーカー、`affinity_key % parallelism` で担当固定（同一 actor は同一スレッド）。キュー満杯は `ExecuteError::Rejected`、停止後は `Shutdown`。状態は `Running → ShuttingDown → Terminated` の CAS
  - `TokioExecutor` — `spawn_blocking` 委譲。shutdown は no-op（ランタイムが所有）
  - `TokioTaskExecutor` — `spawn` 委譲（non-blocking 前提の既定 actor 向け）
- **PORT-5.** `PanicInvokeGuard` は `catch_unwind` で panic を捕捉し、payload（&str / String）を抽出して **`ActorError::escalate`** に変換する（MUST）。panic は Recoverable ではなく上位へのエスカレーションとして扱われる（RFC 0004 の supervision に接続）。
- **PORT-6.** `tokio_actor_system_config(handle)` は default dispatcher（`TokioTaskExecutorFactory`）と blocking dispatcher（`TokioExecutorFactory`、`DEFAULT_BLOCKING_DISPATCHER_ID` へのオプトイン）を分離構成し、`TokioTickDriver` と単調 mailbox clock を組み込む。同期 I/O・CPU 重負荷は blocking 側に置くべきである（SHOULD。rustdoc に宣言）。

### 3.4 embassy adaptor（宣言された挙動）

- **PORT-7.** `EmbassyTickDriver` は `SendSpawner` を必須とし、未設定での `provision` は `TickDriverError::HandleUnavailable` を返す（MUST）。tick 生成タスクと executor 駆動タスクを Embassy task として spawn する。
- **PORT-8.** `EmbassyExecutor<const N: usize>` は固定長 `Channel`（容量 N）+ Signal を持ち、キュー飽和時は**ブロックせず** `ExecuteError::Rejected` を返す（MUST。受付停止後は `Shutdown`）。実行は `EmbassyExecutorDriver::run` が Embassy task 内で Signal を待って 1 ターン最大 64 件のバッチで排出する（executor はキュー投入と wake のみを行い、駆動は driver が担う——crate doc に宣言）。
- **PORT-9.** embassy クレートの公開面は `embassy_actor_system_config` / `EmbassyTickDriver` / `embassy_monotonic_mailbox_clock` のみであり、`Blocker` / `Clock` port の実装は存在しない。

## 4. 不変条件

- **INV-PORT-1**: adaptor は port 実装のみを公開し、core の facade を再輸出しない（PORT-1、lint により機械的に成立）。
- **INV-PORT-2**: いずれの adaptor でも、executor のキュー飽和は呼び出し元へ `ExecuteError::Rejected` として同期的に報告され、暗黙のブロックは発生しない（PORT-4 / PORT-8。RFC 0003 DISP-8 のロールバック契約と対）。
- **INV-PORT-3**: panic した actor ハンドラは（PanicInvokeGuard 構成時）プロセスを落とさず、supervision の Escalate として扱われる（PORT-5）。

## 5. 機械的な問いへの回答

- **空/未設定のとき?** — Tokio handle なし → `HandleUnavailable`。Embassy spawner なし → `HandleUnavailable`（fail-fast）。
- **エラー時の倒れ先は?** — executor 飽和は Rejected（fail-closed、呼び出し元がロールバック）。tick スレッドの join 失敗は警告出力のみ（fail-open）。
- **誰が制御できる入力か?** — resolution / parallelism / N（キュー容量）はシステム構成者が決める。affinity_key は kernel（mailbox PID）が決める。

## 6. Open Questions

| # | 観測した事実 | 質問 | 影響 |
|---|-------------|------|------|
| OQ-PORT-1 | embassy に `Blocker` / `Clock` 実装がない（PORT-9）。`wait_blocking` 系 API や `CircuitBreaker` は embedded で std 実装相当を持たない | kernel 内蔵 `SpinBlocker` で足りるという意図か、未実装ギャップか（RFC 0001 OQ-ARCH-2 と同一事象の詳細） | embedded での終了待ち・circuit breaker の可用性 |
| OQ-PORT-2 | `ThreadedExecutor` はタスクごとに無制限にスレッドを生成する | ブロッキング用途でもスレッド数上限（プール化）が必要ではないか | 高負荷時のリソース枯渇 |

形式化候補（Lean）: `AffinityExecutor` の状態機械（Running / ShuttingDown / Terminated × ワーカーキュー）は「shutdown 後に新規実行なし・投入済みタスクの扱い」を検証する小さな対象。port 契約自体は型レベルの契約なので、Lean 化の主対象は各実装の停止プロトコルである。

## 7. 参照

- `lints/port-adaptor-boundary-lint/SPEC.md`
- RFC 0003（Executor 契約の呼び出し側）、RFC 0006（TickDriver 契約の呼び出し側）
