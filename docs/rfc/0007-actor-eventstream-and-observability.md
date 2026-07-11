# RFC 0007: EventStream と可観測性

| 項目 | 内容 |
|------|------|
| Status | As-built |
| 対象コード | `modules/actor-core-kernel/src/event/`, `actor/actor_ref/dead_letter/` |
| 関連文書 | RFC 0002（Dead Letter の記録元）, `CONTEXT.md`（EventStream / EventBus / EventBus Classification / Message Observability / Dead Letter / Dead Letter Suppression） |
| 最終照合日 | 2026-07-11 |

## 1. 用語

EventStream、EventBus Classification (イベントバス分類)、Dead Letter (デッドレター)、Dead Letter Suppression (デッドレター抑制)、Message Observability。

## 2. 概要

`EventStream` はリプレイバッファ付きのイベントバスであり、ランタイム内部の観測イベント（lifecycle / dead letter / log / mailbox メトリクス / remoting 系 / tick driver）を購読者へ配る。実運用の入口は `EventStreamShared`（`SharedRwLock` ラッパー）で、「バッファ格納 + 購読者スナップショット取得」だけをロック内で行い、コールバック実行はロック外で行うことで再入・デッドロックを回避する。

## 3. 規範仕様

### 3.1 EventStream 本体（宣言された挙動）

- **EV-1.** 公開 API は `subscribe_with_key(key, subscriber)` / `subscribe`（= key `All` の糖衣）/ `subscribe_no_replay` / `unsubscribe` / `publish` であり、subscribe 系は RAII の `EventStreamSubscription`（Drop で自動 unsubscribe）を返す。専用の replay メソッドはなく、リプレイは subscribe 時に「登録とスナップショット取得を同一ロック内で行い、ロック解放後に同期コールバックする」形で実装される。
- **EV-2.** リプレイバッファは FIFO 上限つき（既定 256。システム構築時は EventStream = 256 / Dead Letter = 512 が明示指定される）。あふれは最古から黙って破棄され、通知は発生しない。
- **EV-3.** 分類（EventBus Classification）は `ClassifierKey`（15 値: `Lifecycle` / `Log` / `DeadLetter` / `Extension` / `Mailbox` / `MailboxPressure` / `UnhandledMessage` / `AdapterFailure` / `Serialization` / `RemoteAuthority` / `RemotingBackpressure` / `RemotingLifecycle` / `AddressTerminated` / `TickDriver` / `All`）で行う。`All` 購読者は全イベントを受け取り、特定 key 購読者は該当イベントのみ受け取る（MUST）。
- **EV-4.** 購読者契約は `EventStreamSubscriber::on_event(&mut self, event)`。**コールバックの panic は捕捉されない**（呼び出し元へ伝播し、同一 publish スナップショット内の後続購読者への配送は保証されず、panic した購読者も購読を維持する）——この挙動は rustdoc とテストで宣言されている。mailbox 経由で配送したい場合は非 panic 設計の `ActorRefEventStreamSubscriber`（失敗を `failed_delivery_count` に計上）を使う（SHOULD）。
- **EV-5.** 順序保証の範囲（宣言されている限定）: ある publish の購読者スナップショットはコールバック実行中の subscribe / unsubscribe に影響されない（変更は次の publish から反映）。**スレッドを跨いだ「リプレイがライブより先」という順序は契約に含まれない**。

### 3.2 組み込みイベント（宣言された挙動）

`EventStreamEvent` の variant と発行元:

| variant | 発行元 |
|---------|--------|
| `Lifecycle` | actor lifecycle（RFC 0004 の Started / Restarted / Stopped） |
| `DeadLetter` / `Log` | Dead Letter 記録（記録と同時に Warn ログも発行される） |
| `Mailbox` / `MailboxPressure` | mailbox instrumentation |
| `Serialization` | serialization extension |
| `RemoteAuthority` / `RemotingLifecycle` / `AddressTerminated` | remote 系（kernel は型のみ定義し、発行は remote-core / remote-adaptor-std） |
| `TickDriver` | tick driver provision（RFC 0006 SCH-9） |
| `UnhandledMessage` / `AdapterFailure` | kernel は型のみ定義し、発行は typed 層（RFC 0008） |
| `RemotingBackpressure` | **リポジトリ内に発行元が存在しない（予約）** → OQ-EV-1 |
| `Extension { name, payload }` | 任意コードが使える汎用イベント |

### 3.3 logging（宣言された挙動）

- **EV-6.** ログは EventStream 上の一般イベント（`Log`）として流れる。フィルタは二段: ①システム全体の `LoggingFilter`（publish 前。既定 `DefaultLoggingFilter` は `Trace` 起点で実質すべて通す）②購読者ごとの `LoggerSubscriber.level`（`LoggerWriter` port へ転送する二次フィルタ）。
- **EV-7.** `LogLevel` は `Trace < Debug < Info < Warn < Error` の全順序。補助ファサード（`ActorLogging` / `BusLogging` / `DiagnosticActorLogging` / `LoggingReceive` / `NoLogging`）はすべて `LoggingAdapter` の薄いラッパーである。

### 3.4 Dead Letter の観測（宣言された挙動）

- **EV-8.** Dead Letter の記録は `DeadLetterShared` の hybrid locking（データ変更とイベント発行を分離）で行われ、1 記録につき `EventStreamEvent::DeadLetter` と Warn レベルの `Log` の両方が発行される。
- **EV-9.** `DeadLetterReason::SuppressedDeadLetter` を指定しても発行は抑制されない（reason タグが変わるだけ）。Pekko の `DeadLetterSuppression` marker に相当する自動抑制機構は未実装である → OQ-EV-2。

## 4. 状態機械

本 RFC の範囲に enum ベースの状態機械はない。並行性の中心は「ロック内の格納・スナップショット / ロック外のコールバック」という 2 相構造（§3.1）である。

## 5. 不変条件

- **INV-EV-1**: publish は購読者ゼロでも失敗しない（バッファ格納のみ行われる no-op）。
- **INV-EV-2**: 1 回の publish で、スナップショットに含まれる購読者以外へ配送されることはない（コールバック中の購読変更は次回から反映）。
- **INV-EV-3**: リプレイバッファの長さは常に capacity 以下である（push 後の即時 trim により成立）。
- **INV-EV-4**: Dead Letter の 1 記録は DeadLetter イベントと Log イベントをちょうど 1 つずつ発行する（EV-8）。

## 6. 機械的な問いへの回答

- **空/未設定のとき?** — 購読者ゼロの publish は無害（INV-EV-1）。フィルタ未設定はすべて通す（EV-6）。
- **エラー/取得失敗のとき?** — 購読者 panic は伝播し、購読は維持される（EV-4。fail-open：観測層の失敗が発行側スレッドを巻き込む）→ OQ-EV-3。
- **同時に 2 つ来たら?** — 同時 publish はバッファ格納のみ直列化され、コールバックは並行に走りうる。相互順序の保証はない（EV-5）。
- **境界はどっち向き?** — バッファは「格納してから溢れ分を落とす」（一瞬 capacity+1 になってから trim。外部観測には現れない）。

## 7. Open Questions

| # | 観測した事実 | 質問 | 影響 |
|---|-------------|------|------|
| OQ-EV-1 | `RemotingBackpressureEvent` は型・分類キーとも定義済みだが、リポジトリ全体で発行箇所がない | remote backpressure の配線は未完か、外部（将来の adaptor）用の予約か | backpressure の観測可能性 |
| OQ-EV-2 | Dead Letter Suppression は reason タグのみで、自動抑制も発行スキップも存在しない（EV-9）。`CONTEXT.md` は Dead Letter Suppression を独立概念として定義している | suppression の実装（marker trait / 発行抑制）は計画済みか | 高頻度 dead letter（terminated への tell 等）のノイズ |
| OQ-EV-3 | 購読者 panic が発行側へ伝播する（EV-4） | ランタイム内部の発行点（lifecycle / dead letter 経路）が利用者購読者の panic に巻き込まれる設計は意図か | 観測コードの欠陥がコア経路を停止させうる |

形式化候補（Lean）: 「ロック内スナップショット + ロック外コールバック」の 2 相 publish と subscribe / unsubscribe のインターリーブに対する INV-EV-2 の検証。バッファ trim（INV-EV-3）は単純な列不変条件として練習台に適する。

## 8. 参照

- RFC 0002 §7（DeadLetterReason と記録元）、RFC 0008（UnhandledMessage / AdapterFailure の発行元）、RFC 0009（LoggerWriter / EventStreamSubscriber の adaptor 実装）
