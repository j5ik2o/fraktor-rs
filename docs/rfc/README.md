# fraktor-rs RFC（as-built 仕様）

このディレクトリは、fraktor-rs の**現行実装から吸い出した仕様（as-built specification）**を RFC スタイルで記録する。各 RFC は「実装の主張・検査結果・ドメインへの確認質問」の台帳であり、コードが既に持っている契約・不変条件・状態機械を、実装を読まずに参照できる形へ固定することを目的とする。

手法は形式手法の実践プレイブック（実装を事実上の仕様として抽出し、不変条件を名前付きで宣言し、反例志向で記述する）に基づく。RFC は証明ではない。**実装が何を主張しているか**と、**まだ意図かバグか確定していない点（Open Questions）**を分けて記録する。

## 既存ドキュメントとの役割分担

| 文書 | 役割 | RFC との関係 |
|------|------|--------------|
| `openspec/specs/*` | public contract の変更単位の正本 | RFC は複製しない。該当 capability を参照で示す |
| `.kiro/specs/*` | feature 実装（requirements / design / tasks）の正本 | RFC は複製しない。設計判断の出典として参照する |
| `docs/adr/*` | 不可逆な設計判断 | RFC は ADR と矛盾してはならない。該当 ADR を参照する |
| `CONTEXT.md` | ドメイン語彙の正本 | RFC は canonical term を使う。用語の新定義はしない |
| `docs/gap-analysis/*` | 参照実装（Pekko 等）との差分記録 | RFC は「現行のあるべき挙動」を書き、差分は gap-analysis に委ねる |

RFC が固有に持つのは、**サブシステム横断の現行仕様・名前付き不変条件・状態機械・確認質問の台帳**である。他文書の内容を重複コピーせず、参照で繋ぐ（`docs/plan/reverse-kiro-domain-docs.md` の運用に従う）。

## 更新規約（陳腐化対策）

- 各 RFC はヘッダに「対象コード」と「最終照合日」を持つ。
- 対象コードのふるまいを変える PR では、該当 RFC の該当節を見直し、変更するか、変更不要ならヘッダの最終照合日を更新する。
- 実装と RFC が食い違ったまま放置される状態を許容しない。追従できない RFC は削除ではなく Status を `Stale` に変えて理由を残す。

## 体裁規約

- 言語は日本語。初出のドメイン用語は `English Term (日本語名)` を優先し、`CONTEXT.md` の canonical term に従う。
- 規範語は RFC 2119 に倣い **MUST / MUST NOT / SHOULD / MAY** を日本語文中で使う。規範文には根拠となる実装位置（`path/to/file.rs` と必要なら行番号）を添える。
- 「宣言された挙動」（rustdoc・テスト・ガード節が明示するもの）と「暗黙の挙動」（default 値・エラー分岐・順序依存・信頼境界から読み取れるもの）を区別して記述する。
- 不変条件は `INV-<AREA>-<n>` の ID で名前付き宣言する（例: `INV-MB-1`）。AREA は RFC ごとに定める短いコード。
- 状態機械は「状態の列挙 → イベント/遷移関数 → 遷移表」の順で、肯定的・決定的に記述する。
- Open Questions は「観測した事実 → 質問（意図かバグか）→ 影響」の3点で記録する。形式化候補（Lean 等の証明支援系や TLA+ / Z3 でモデル化する価値がある状態機械・不変条件）があれば付記する。

### 各 RFC の標準構成

1. ヘッダ（Status / 対象コード / 関連文書 / 最終照合日）
2. 用語
3. 概要
4. 規範仕様（宣言された挙動・暗黙の挙動）
5. 状態機械
6. 不変条件
7. 機械的な問いへの回答（空・エラー・境界・同時性・停止性・合意のうち該当するもの）
8. Open Questions
9. 参照

Status は `As-built`（現行実装と照合済み）、`As-built (reference)`（参照実装 Pekko と照合済み）、`Stale`（実装との照合が追従できていない）のいずれか。

## Pekko ミラー版

`pekko/` 配下には、参照実装 Apache Pekko（`references/pekko`）の同一サブシステムを同じテンプレート・同じ番号で吸い出したミラー RFC を置く。番号が一致する fraktor 版と Pekko 版は同じ領域を扱い、各 Pekko 版は「fraktor-rs との差分」節で意味論の相違を明示する。照合対象は `references/pekko` に vendoring されたコミットである（各 RFC ヘッダに記載）。

### ミラー版の簡約規定

ミラー RFC は **Pekko の意味論のみ**を記述し、fraktor-rs への言及（差分表・対応モジュール・parity 評価）を含めない。fraktor との比較は fraktor 側 RFC（Open Questions 等）と `docs/gap-analysis/` の責務である。標準構成のうち「用語」「機械的な問いへの回答」「Open Questions」は省略してよい（用語は fraktor 版と共通、意図確認の宛先は上流 OSS になるため、Pekko 比較から生じた確認事項は fraktor 側 RFC の Open Questions に集約する）。番号対応と差分の要約は本 README の索引だけが持つ。

### ミラー版のスコープ宣言

対象は `actor` / `actor-typed` 2 モジュールの意味論である。以下は対象外とする。

| 対象外 | 理由 |
|--------|------|
| `util/` | 内部ユーティリティ（意味論の担い手ではない） |
| `japi/`, `javadsl/`, `event/japi/`, `AbstractActor` 等の `Abstract*` | Java API 投影。意味論は Scala 側と同一（pekko-0001 P-11） |
| `internal/jfr/` | JVM Flight Recorder 連携（観測基盤であり actor 意味論ではない） |
| スレッドプール構築の実装詳細 | `ForkJoinExecutorConfigurator` 等の構成式は pekko-0009 に要点のみ収載 |
| 単純値型・内部配管 | `Done` / `NotUsed` / `PekkoVersion` 等の値型と、公開面の節で意味論を捕捉済みの `*Impl` 配管クラスは個別収載しない |

`io/`（TCP / UDP / DNS）は fraktor に対応 RFC が存在しないため、番号対応の枠外として [pekko-0011](pekko/0011-actor-io.md) に参照専用で収載する。

## 索引

| RFC | タイトル | 範囲 |
|-----|---------|------|
| [0001](0001-actor-architecture-overview.md) | actor アーキテクチャ概観 | 4 クレート構成、port-adaptor 依存方向、guardian 階層、Pekko parity 方針 |
| [0002](0002-actor-messaging-and-mailbox.md) | メッセージングと mailbox | AnyMessage / Envelope、メッセージキュー群、MailboxScheduleState、Dead Letter |
| [0003](0003-actor-dispatch-and-executor.md) | dispatch と executor | MessageDispatcher、throughput、再入防止、at-most-once tell |
| [0004](0004-actor-lifecycle-and-supervision.md) | ライフサイクルと supervision | ActorCell、SystemMessage、ChildrenContainer、SupervisorStrategy |
| [0005](0005-actor-deathwatch-and-termination.md) | DeathWatch と終了 | watch/unwatch、Terminated 配送、TerminationState、Coordinated Shutdown |
| [0006](0006-actor-scheduler-and-tick.md) | スケジューラと tick | SchedulerCore、TickDriver port、Receive Timeout、timers、classic FSM |
| [0007](0007-actor-eventstream-and-observability.md) | EventStream と可観測性 | EventStream、EventBus Classification、Dead Letter Suppression、logging |
| [0008](0008-actor-typed-layer.md) | typed 層 | Behavior、typed↔untyped 接続、Signal、Receptionist、PubSub |
| [0009](0009-actor-port-adaptor-contract.md) | port-adaptor 契約 | port 一覧、std / embassy 実装対比、境界 lint |
| [0010](0010-actor-routing-serialization-patterns.md) | routing・serialization・patterns | Router、SerializationRegistry、ask / retry / Circuit Breaker |

### Pekko ミラー版索引

| RFC | タイトル | fraktor 版との主な差分 |
|-----|---------|----------------------|
| [pekko-0001](pekko/0001-actor-architecture-overview.md) | アーキテクチャ概観 | JVM 固定・設定駆動、dungeon trait mixin |
| [pekko-0002](pekko/0002-actor-messaging-and-mailbox.md) | メッセージングと mailbox | あふれは送信側破棄 + pushTimeOut のみ（DropOldest / Grow なし） |
| [pekko-0003](pekko/0003-actor-dispatch-and-executor.md) | dispatch と executor | 二段階送信なし、BatchingExecutor、既定は fork-join |
| [pekko-0004](pekko/0004-actor-lifecycle-and-supervision.md) | ライフサイクルと supervision | system message stash、perpetrator 限定 cause、UID 照合、PoisonPill は user 順 |
| [pekko-0005](pekko/0005-actor-deathwatch-and-termination.md) | DeathWatch と終了 | terminate は Coordinated Shutdown を経由（12 フェーズ） |
| [pekko-0006](pekko/0006-actor-scheduler-and-tick.md) | スケジューラと時間 | 内蔵スレッド、遅延 0 は即時実行、専用 ReceiveTimeout メッセージ |
| [pekko-0007](pekko/0007-actor-eventstream-and-observability.md) | EventStream と可観測性 | Class 階層分類、リプレイなし、DeadLetterSuppression 実装済み |
| [pekko-0008](pekko/0008-actor-typed-layer.md) | typed 層 | タグ式 Behavior、無検査ダウンキャスト、DeathPactException は throw |
| [pekko-0009](pekko/0009-actor-port-adaptor-contract.md) | 実行環境接続 | port 抽象なし（設定 + FQCN リフレクション） |
| [pekko-0010](pekko/0010-actor-routing-serialization-patterns.md) | routing・serialization・patterns | リング + 仮想ノード、gracefulStop は watch ベース、CircuitBreaker に callTimeout |
| [pekko-0011](pekko/0011-actor-io.md) | io（TCP / UDP / DNS） | fraktor に対応 RFC なし（参照専用。番号対応の枠外） |
