# ギャップ分析: mailbox-dispatcher-refactor-pekko

## 1. 現状調査サマリー
- **Mailbox/Dispatcher**: `modules/actor-core/src/mailbox/base.rs` と `modules/actor-core/src/dispatcher/dispatcher_core.rs` が system/user キューの既存制御を担うが、Pekko 相当の system queue CAS、Scheduled/Suspended/Running/Tombstone などの状態遷移は未実装。DispatcherState も Idle/Running のみ。
- **キュー基盤**: utils-core の `VecRingBackend` + `WaitQueue` が MPSC／Block 戦略と非同期 Future (`MailboxOfferFutureGeneric`) を提供。Deque push/pop を公開しておらず Stash 要件を満たせない。
- **Config/Props**: `PropsGeneric` は mailbox policy と executor 参照を持つだけで、dispatcher/mailbox ID ルックアップ、`RequiresMessageQueue` 検証、Pekko 互換キー読み替えなどは不在。
- **Observability**: `MailboxInstrumentation` が EventStream へ `MailboxMetricsEvent` を publish し、DeadLetter も理由つきで流せるが、dispatcher dump API や mailbox pressure イベントの定義は無い。
- **STD ブリッジ**: `actor-std` 側は `TokioExecutor` が `spawn_blocking` するのみで、waker/notify 連携や RejectedExecution リトライ、backpressure telemetry の橋渡しロジックは未着手。

## 2. 要件 × 資産マッピング

| 要件 | 既存箇所 | 状態 | ギャップ/補足 |
| --- | --- | --- | --- |
| R1: システムメッセージ優先 | `mailbox/base.rs` | Partial | FIFO で system キュー優先だが、CAS ベース systemEnqueue/systemDrain、優先度リスト、DeadLetter フォールバック再試行が未実装。|
| R2: Dispatcher スケジューリング | `dispatcher/base.rs`, `dispatcher_core.rs` | Missing | registerForExecution ヒント、Scheduled フラグ制御、RejectedExecution 2回リトライ、Idle へのロールバックがない。|
| R3: バックプレッシャー・Stash | `mailbox/base.rs`, `utils-core` queue | Missing | Block Future は存在するが `MailboxPressure` publish、Deque API、Stash capacity エラー、DeadLetter 遷移が未実装。|
| R4: Telemetry & DeadLetter | `mailbox_instrumentation.rs`, `dead_letter/*` | Partial | メトリクス publish は可、dispatcher dump API や mailbox pressure イベント種別が未定義。|
| R5: Mailbox 状態機械 | `dispatcher_state.rs`, `mailbox/base.rs` | Missing | Idle/Suspended フラグのみ。Scheduled/Suspended/Running/Closed 状態遷移、NeedReschedule/Idle 戻し手続きが必要。|
| R6: API 駆動解決 | `props/base.rs`, `dispatcher_config.rs` | Missing | Mailboxes/Dispatchers レイヤ、DispatcherDescriptor、`RequiresMessageQueue`/`ProducesMessageQueue` 照合、Pekko 互換キー解決が無い。|
| R7: STD 実行器ブリッジ | `actor-std/src/dispatcher/*` | Missing | Tokio/tread executor への waker 連携、RejectedExecution リトライ、backpressure イベント送出が未実装。|
| R8: Future Handshake | `mailbox/base.rs`, `dispatcher_sender.rs` | Partial | Offer future は活用中だが Poll future は未使用、STD 側 waker 変換も必要。|
| R9: utils-core Queue 能力 | `mailbox/mailbox_queue_handles.rs`, `utils-core/collections/queue` | Missing | Deque push_front/pop_front API を公開していない。Block future/OverflowPolicy の要件も契約化されていない。|

## 3. 実装アプローチ

### Option A: 既存コンポーネント拡張
- `MailboxGeneric`/`DispatcherCore` に Pekko 互換機能を順次追加し、utils-core のバックエンドを直接拡張。
- **メリット**: 既存 API とテストを再利用、変更多数でもファイル数は据え置き。
- **デメリット**: 巨大ファイル化と結合度上昇。Stash/Config 周りの新規責務が増え、SRP を損なうリスク。

### Option B: 新コンポーネント新設
- `system_mailbox_queue.rs`、`dispatcher_registry.rs`、`mailbox_deque_backend.rs` など新モジュールで責務分離。Config 解決は `modules/actor-core/src/mailbox_resolver/` 的ディレクトリに追加。
- **メリット**: 責務分断・単体テスト容易。Pekko の構造に近い層構造を再現できる。
- **デメリット**: 既存コードからの移行が大きく、API 変更が広範囲に波及。教育コスト高。

### Option C: ハイブリッド（推奨）
- 第1段階で `MailboxGeneric`/`DispatcherCore` に state machine, CAS, hint API, future ハンドシェイクを実装。並行して `dispatchers.rs` / `mailboxes.rs` のラッパを新設し、Props から段階的に切替。
- `utils-core` には Deque backend を追加し、Stash 対応 actors のみ新 backend を opt-in できるようにする。
- **メリット**: 互換性を保ちつつ段階移行・フェーズ投入可能。危険箇所を個別に feature flag で切替えられる。
- **デメリット**: フェーズ管理が複雑。旧実装との共存期間が長くなる。

## 4. 複雑度・リスク
- **Effort**: **XL (2+ 週間)** — Mailbox/Dispatcher/Props/utils-core/actor-std/Docs の複数クレートに手を入れ、no_std/STD を両対応で保守する必要がある。
- **Risk**: **High** — lock-free 処理や executor 連携の変更でデッドロック・過負荷・監視抜けが起きやすい。Tokio/no_std 双方のテストスイート整備も追加で必要。

## 5. Research Needed
1. **Deque ベース backend 設計**: lock-free vs mutex、push_front の API 露出方針。
2. **registerForExecution ヒント実装**: no_std 環境での優先度セマフォ／Atomic 状態管理の安全設計。
3. **Stash capacity & RuntimeConfig API**: Stash 関連設定をどこで保持・検証するか（Props? SystemState?）。
4. **Dispatcher dump/Telemetry**: dump のデータモデル、EventStream との連携仕様。
5. **STD executor リトライ戦略**: `spawn_blocking` 以外の適切な waker/notify をどう抽象化するか。

## 6. 推奨アクション（設計フェーズ入力）
- Option C を前提に設計を進め、system queue / state machine / utils-core Deque backend / config resolver / STD ブリッジをフェーズ分割して定義する。
- 上記 Research Needed を設計タスクに紐付け、PoC または doc 調査の責任者・期限を決める。
- EventStream/API の互換を壊さないよう、Telemetry 拡張に feature flag を用意する方針を検討する。

---
*生成日時: 2025-11-10T15:00:21Z.*
