## Why

gap-analysis `MB-M1` (medium) として、kernel `Mailbox::run()` が `_throughput_deadline:
Option<Duration>` 引数を受け取りながらも `_` プレフィックスで **未使用**
(`base.rs:256, 294` に `// Deadline support is added in a follow-up change` と明記) であるため、
Pekko `Mailbox.scala:261-278` が規定する throughput **deadline** による yield
が fraktor-rs では発火しない。結果として 1 メッセージの処理が長時間かかるアクターが
throughput 本数を消化するまで dispatcher を独占し、他アクターの fairness が崩れるリスクが残る。

Pekko 準拠の mailbox 実行契約 ( `processMailbox` ループで `left > 1 && (nanoTime -
deadlineNs) < 0` の合成条件を enforce) を完全に再現し、fraktor-rs の throughput enforcement を
「本数 + deadline」双方に基づかせる。

## What Changes

- **BREAKING (観測挙動)**: `Mailbox::run(throughput, _throughput_deadline)` の `_throughput_deadline`
  プレフィックスを削除、ループ内で実際に使用する。
  これまで静かに無視されていた `DispatcherConfig::with_throughput_deadline(Some(...))` 設定が
  実際に yield を引き起こすようになる。
- **BREAKING (観測挙動)**: `process_mailbox` のループ条件を Pekko `Mailbox.scala:275` に合わせ
  `left > 0` 単独から `left > 0 && (deadline is None || monotonic_now() < deadline_at)` に拡張。
- **ADDITIVE (公開型)**: `MailboxClock = Arc<dyn Fn() -> Duration + Send + Sync>` 型 alias を
  新設 (kernel 外に `pub` 公開)。
- **ADDITIVE (kernel 内部 API)**: `MailboxSharedSet` (`pub` struct、内部は `pub(crate)` API のみ)
  に `clock: Option<MailboxClock>` field + `pub(crate) with_clock()` / `pub(crate) clock()`
  を追加。外部クレートからは不可視。既存の `Mailbox::new*` factory 8 本の signature は維持
  される (clock は `MailboxSharedSet` 経由で自動注入)。
- **BREAKING (kernel 内部 API)**: `MailboxSharedSet::new` の `const fn` 修飾子を削除
  (`Arc<dyn Fn()>` 含む field は const-context 構築不可のため)。呼び出し経路は kernel 内部のみ。
- kernel `Mailbox` に **monotonic 時刻ソース** を注入する経路を整備する。no_std core では
  `MailboxClock = Arc<dyn Fn() -> Duration + Send + Sync>` 型 alias を定義、std adaptor 側で
  `Instant::now()` 由来の closure を注入。
- deadline 未設定 (`None`) の場合は従来の throughput-only 挙動を完全に保つ。
- Pekko `Mailbox.scala:261-278` との行単位対応を rustdoc コメントで明記。
- 新規テストで Pekko deadline 契約を pinned する:
  - 長時間処理アクターが throughput 未消化でも deadline 超過で yield する
  - deadline `None` では throughput を消化しきるまで続行する
  - throughput `1` の境界で deadline の有無が挙動を変えないこと (Pekko `left > 1` の意図)
  - `deadline = Some(Duration::ZERO)` は 1 件処理後に即 break する挙動
- `DispatcherConfig::throughput_deadline` は既に `Option<Duration>` として伝播済みのため、
  dispatcher 側の変更は最小 (`Mailbox::run` 内部のみ)。

## Capabilities

### New Capabilities

- `pekko-mailbox-throughput-deadline`: mailbox の `run()` / `process_mailbox` における
  throughput deadline enforcement、monotonic 時刻ソースの扱い、deadline 未設定時の
  throughput-only fallback、Pekko `Mailbox.scala:261-278` との行単位対応契約を定義する。

### Modified Capabilities

- なし (既存 capability は改変しない。deadline enforcement は新規 capability として独立させる)

## Impact

**影響コード:**
- `modules/actor-core/src/core/kernel/dispatch/mailbox/base.rs` — `run()` / `process_mailbox`
  へ deadline enforcement を追加
- `modules/actor-core/src/core/kernel/dispatch/mailbox/` 配下のテスト類 — deadline 契約テスト追加
- `modules/actor-core/src/core/kernel/dispatch/dispatcher/message_dispatcher_shared.rs:289-305`
  — `run()` 呼び出しで `now` をどう渡すか (clock 注入経路) に関する small refactor
- `modules/actor-core/src/core/kernel/system/actor_system.rs` 付近 — `monotonic_now()`
  の公開 (既に SP-M1 で導入済みであれば流用)

**公開 API 変更:**
- `Mailbox::run()` シグネチャは維持 (`_throughput_deadline` の `_` を削除するだけで型は既存 `Option<Duration>`)。
  内部で時刻を取得する必要があるため、**新規 clock 注入メカニズム** の設計が必要。
  詳細は design.md で検討する。

**依存関係 / システム:**
- 他モジュールへの影響はない (kernel 内閉じた変更)。
- `DispatcherConfig::with_throughput_deadline(Some(...))` を設定している既存利用箇所は
  今まで「静かに無視されていた」設定が有効化されるため、**挙動変更** となる。CLAUDE.md
  方針に従い、この破壊性は許容する。

**スコープ非対象:**
- MB-M2 (BoundedDequeBasedMailbox / BoundedControlAwareMailbox の追加実装) は別 change。
- MB-M3 (blocking push-timeout、非同期 Rust で設計外) は skip 対象。
- AC-M1 (PinnedDispatcher 排他ガード), AC-M2 (dispatcher config alias 解決) は別 change。
