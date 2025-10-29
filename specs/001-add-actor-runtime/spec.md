# 機能仕様: セルアクター no_std ランタイム初期版

**ブランチ**: `[001-add-actor-runtime]`
**作成日**: 2025-10-29
**ステータス**: Draft
**入力**: ユーザ要望: "最初のスペックです。原則に従ってください。Rust(no_std)で動作可能なアクターシステムを作りたい。ActorSystem, Actor, ActorRef, Supervisor, Mailbox, MessageInvoker, Dispatcher, ActorCell, ActorFuture, Deadletter, ActorContext, EventStream, Pid, Props, Behaviorなどを搭載した初期版の実装を作りましょう。Queueはutils-coreのAsyncQueueを使って。asyn fnは必要最低限。全体を汚染させない。循環参照をさけて。実行できるサンプルコードも作って。この初期版を土台に機能拡張できるようにして"

> protoactor-go の Minimal Actor サンプルと Apache Pekko Classic の基本アクターライフサイクルを参照し、Rust の `no_std` 制約下へ転写する指針を各節で明記する。

## ユーザーストーリーとテスト（必須）

### ユーザーストーリー1 - 最小アクターを起動してメッセージを処理する（優先度: P1）

組込み向けアプリ開発者として、protoactor-go の `examples/spawn` 相当のシナリオを Rust `no_std` 環境でも再現できるように、ActorSystem と ActorRef だけでメッセージ送受信が完結し、メッセージ本体は `dyn core::any::Any` を包む `AnyMessage` によって未型付けで扱える最小構成を提供してほしい。Apache Pekko の「Quick Start」で示される `ActorRef.tell` の遷移を Rust で確認できることを期待する。

**優先度の理由**: ランタイム採用可否を判断するための最初のユーザバリューであり、他要素の前提となる。
**独立テスト**: 提供されるサンプルコードをターゲットボード向けの `cargo` ビルド（`no_std + alloc`）で実行し、メッセージ送受信ログをシリアル出力で検証する E2E テスト。

**受け入れシナリオ**:

1. **前提** ランタイム初期化時に ActorSystem とルート Props が登録済み、**操作** 単一アクターを spawn して `ActorRef` に `AnyMessage::new(Ping)` を送信、**結果** アクターが `Actor::receive` 内で `downcast_ref::<Ping>()` を通じて `Ping` を取り出し、`Pong` を返信してサンプルが完走する。
2. **前提** メッセージ処理中にキューへ 32 件の要求が積まれている、**操作** Mailbox が protoactor-go の `DefaultDispatcher` 相当の順序で処理、**結果** 全メッセージが FIFO 順守で消化され、バックプレッシャー無しで完了ログが出力される。

---

### ユーザーストーリー2 - 階層的な監視と再起動ポリシーを適用する（優先度: P2）

ランタイム統合エンジニアとして、Apache Pekko のスーパービジョンツリーを参考に、アクターの失敗を捕捉し `Supervisor` が再起動戦略を適用できるようにしたい。protoactor-go の `OneForOneStrategy` をモデルとして Rust 向けに再設計されたポリシーを設定し、負荷試験時にも自動回復できる必要がある。

**優先度の理由**: 安定稼働に必須の信頼性機能であり、初期 adopters 向け PoC に含める。
**独立テスト**: 疑似エラーを発生させるテストアクターを用いた統合テストで、Supervisor のポリシー適用回数と停止ログを検証する。

**受け入れシナリオ**:

1. **前提** Supervisor が `Restart` ポリシーを設定済み、**操作** 子アクターがハンドラ内で意図的にパニックを発生、**結果** Supervisor がエラーを捕捉してアクターを即時再起動し、再起動カウンタが仕様で定める最大値未満でリセットされる。
2. **前提** Supervisor が `Escalate` ポリシーを設定済み、**操作** 致命的なエラーコードを返すメッセージを処理、**結果** 上位ツリーに例外が伝播し、ActorSystem が Deadletter 経由で通知を行う。

---

### ユーザーストーリー3 - 動作を観測して運用判断を下す（優先度: P3）

プラットフォーム運用担当として、protoactor-go の `EventStream` と Pekko の `DeadLetter` ログを参考に、アクター間通信の健全性をリアルタイムで観測したい。Deadletter と EventStream を Subscribe し、未配達メッセージや状態遷移を把握できる仕組みを提供することで、運用ダッシュボードに統合できるようにする。

**優先度の理由**: 運用品質を可視化し、初期導入ユーザの信頼を確保するため。
**独立テスト**: イベント購読 API を使った結合テストで、意図的に不達メッセージを生成し Deadletter の記録内容と EventStream 通知件数を検証する。

**受け入れシナリオ**:

1. **前提** EventStream に外部購読者が登録済み、**操作** 任意のアクターが `Behavior::become` で状態遷移、**結果** 状態遷移イベントが購読者に配送され、タイムスタンプと PID が一致して記録される。
2. **前提** 宛先不明 PID に `tell` を送信するテストが走っている、**操作** メッセージを送信、**結果** Deadletter が未配達メッセージと原因を記録し、EventStream にも転送される。

---

### 境界条件・例外

- `no_std` + `alloc` 環境で利用可能なことを前提とし、標準ライブラリ固有 API への依存は禁止する。
- すべてのアクター間メッセージは `AnyMessage` を経由して `dyn core::any::Any` として扱い、Typed アクター API は後続フェーズでレイヤー追加する方針とする。
- 送信時には所有型 `AnyOwnedMessage` に変換して Mailbox に格納し、取り出し時に借用型 `AnyMessage<'_>` を再構成してアクターへ渡す。所有型は `modules/utils-core::sync::ArcShared` を利用して複製を避ける。
- Mailbox は制御用 System メッセージとユーザメッセージを優先度付きで扱い、停止・再開時でも System メッセージが処理される設計を必須とする。
- Mailbox の内部構成は System 用と User 用に 2 本のキューを持ち、バックエンドには `modules/utils-core::collections::queue::async_queue` で提供される `AsyncQueue` ファミリーを採用する。System キューの処理が優先され、空の場合のみ User キューを処理する。`suspend()` は dequeue を停止し、`resume()` で再開する。外部に公開する API は同期呼び出しを維持しつつ、`AsyncQueue` のカスタム Future を Dispatcher 内の協調ポーリング（自前 executor）で駆動し、`async fn` 依存を避ける。
- `AsyncQueue` が提供する `offer_blocking` / `poll_blocking` はそれぞれ `OfferFuture` / `PollFuture` を返し、所有権を保持したまま待機登録・解除を制御する。Future が `Poll::Pending` を返す際はロックを開放し、drop 時には WaitQueue から確実に解除される設計とする（詳細は `docs/mailbox-spec.md` を参照）。
- ランタイム API は借用ベースのライフタイム設計を基本とし、ヒープ確保が必要な処理は事前に計測計画と再利用戦略を記載する。
- Actor トレイトは `pre_start`, `post_stop`, `receive` を提供し、`pre_start` でリソース初期化、`post_stop` で解放ができるようにする。`receive` は `Result<(), ActorError>` を返却し、`Err` の場合は Supervisor が再起動戦略に基づいて扱う。`panic!` などスタックを巻き戻せない致命的障害は ActorSystem が介入せず、アプリケーション側でリセットやフォールバックを実装することを前提とする。
- 同名・同型のエラーや trait を異なる責務に使い回すことを禁止する（例: `ActorError` はアクター実行時の失敗に限定し、Mailbox 送信時の背圧エラーには別の `SendError` を用意する）。
- アプリケーションのブートストラップは ActorSystem を `Props::new(user_guardian_factory)` で生成し、以降のアクター生成は **必ず** ガーディアン（または子アクター）が `ActorContext::spawn_child` を通じて行う。`ActorSystem` 本体にトップレベルの汎用 `spawn` API は公開しない。
- ActorSystem はブート時に登録したユーザガーディアンへアクセスするための `user_guardian_ref()`（名称は同等の意図を満たせばよい）を提供し、エントリポイントとなるメッセージ（例: `Start`) をその `ActorRef` に対して `tell` することでアプリケーションを起動する。
- AsyncQueue が満杯の場合は protoactor-go の `BoundedMailbox` を参考にバックプレッシャーをかけ、送信 API は明示的な失敗結果を返す。
- `Block` ポリシーは no_std 向けに WaitNode を用いた待機を採用し、Busy wait を避ける。enqueue 側は `AsyncQueue` の `offer_blocking`、dequeue 側は `poll_blocking` が返すカスタム Future を利用して待機し、Mailbox 再開時に待機ノードへ通知する。初期リリースではポリシー定義と待機ハンドラ API を公開し、実際のブロッキング挙動は協調ポーリング上で段階的に実装する。
- Supervisor により再起動回数が上限を超えた場合、ActorSystem はアクターを停止させ Deadletter と EventStream へ必ず通知する。
- Actor は子アクターを生成し、Supervisor 戦略に基づいた親子ツリーを形成できることを前提とする。親は子アクターのライフサイクルイベントを EventStream 経由で監視できる必要がある。
- ActorSystem の初期化時にユーザガーディアン（root actor）のインスタンスと Props を受け取り、そのコンテキストから子アクターを生成してシステム全体を構築できるようにする。
- Mailbox は Bounded / Unbounded を切り替え可能であり、Bounded 時は容量閾値で背圧またはドロップポリシーを適用し、Unbounded 時は組込みのメモリ制約を超えないよう監視メトリクスを提供する。
- Classic Akka の `sender()` を提供せず、応答が必要なメッセージは `reply_to: ActorRef` などのフィールドを持つリクエスト・リプライパターンを利用することを前提とする。
- アクターには一意な名前を付与でき、未指定の場合は ActorSystem が自動生成した名前を割り当てる。名前は PID レジストリから逆引きできる。
- サンプルコードは PC 上のホストビルドと LLVM 目標ボード（例: RISCV）など少なくとも 2 つのターゲットでビルドできること。

## 仮定と依存関係

- `modules/utils-core` の `AsyncQueue` および `Shared` 抽象が既存実装として利用可能であり、追加の同期プリミティブを導入しない。
- メッセージ表現は `AnyMessage` 構造体と `dyn core::any::Any` のみに依存し、Typed メッセージ/アクターは拡張レイヤーが担う。
- メッセージシリアライズは初期版ではトレイト境界のみ定義し、具象実装は後続フェーズで拡張する。
- ロガー／タイマーなどの周辺機能は既存ユーティリティを再利用し、ActorSystem 側ではインターフェイスだけを提供する。EventStream を介した Logger 購読者で観測できることを前提とする。
- ライフタイム重視の API 設計を維持するため、借用で表現できるデータに対して所有権移動や追加アロケーションを求めないことを前提とする。

## 要件（必須）

### 機能要件

- **FR-001**: ActorSystem は 1 回の初期化呼び出しでルート Context・PID 名前空間・Supervisor ツリーを確立し、protoactor-go の RootContext と同等のブートストラップ API（ユーザガーディアンを起点とするアクター生成）を `no_std` 下で提供しなければならない。
- **FR-002**: Actor トレイトは Classic スタイルのビヘイビア遷移メソッド（`receive`, `become`, `unbecome` 相当）を提供し、Apache Pekko が定義する動的ビヘイビア切替フローを Rust のライフタイム制約に合わせて実行できなければならない。Typed Behavior (Pekko Typed) は後続フェーズで別レイヤーとして導入する。
- **FR-003**: ActorRef と Pid は一意識別子を保持し、同一アクターへの再解決が O(1) で完了するルックアップテーブルを ActorSystem 内に提供しなければならない。
- **FR-004**: Mailbox は `modules/utils-core::collections::queue::async_queue`（以下 AsyncQueue）を内部キューとして用い、protoactor-go の Mailbox 処理順序と同様の FIFO 保証を仕様化しなければならない。AsyncQueue が提供する `offer_blocking` / `poll_blocking` のカスタム Future を利用し、`Block` ポリシーや `suspend`/`resume` 制御に必要な協調ブロッキングを実現すること。`VecDeque` など簡易キューによる仮実装は禁止とする。
- **FR-005**: Dispatcher と MessageInvoker は メッセージ取得→ビヘイビア呼び出し→ポスト処理の段階を分離し、Pekko の `Dispatcher` 設計を参考に同期／非同期両モードを後日拡張可能な形でインターフェイス化しなければならない。アクター起動や `pre_start` 呼び出し、`run_until_idle` 相当の処理は Dispatcher 側の責務とし、Mailbox の enqueue 経路に入れないこと。
- **FR-006**: Supervisor は `OneForOne` と `AllForOne` の 2 種類以上の戦略を持ち、再起動回数制限・遅延・エスカレーション条件を設定できる API を提供しなければならない。
- **FR-007**: ActorCell と ActorContext は アクターのライフサイクル状態（初期化中／稼働中／停止）と親子関係を保持し、サンプルコードで状態遷移を EventStream へ通知できなければならない。
- **FR-008**: ActorFuture は 非同期返信のための簡易 Future/Promise API を提供し、`async fn` を利用せずにポーリングベースの完了検知ができるようにしなければならない。
- **FR-009**: Deadletter と EventStream は 100% の未配達メッセージと監視イベントを記録し、購読インターフェイス（Subscribe/Unsubscribe/Publish）を提供しなければならない。Deadletter は失敗した `AnyOwnedMessage`（payload・metadata・`reply_to` を含む）を保持し、EventStream にも同情報を含む通知を流すことで再送・解析に利用できなければならない。
- **FR-010**: Props は アクター生成時のファクトリ・Mailbox 設定・Supervisor 紐付けを宣言的に構築でき、protoactor-go の `Props` 相当 API を Rust の所有権モデルに合わせて記述しなければならない。
- **FR-011**: すべてのメッセージは `AnyMessage` にカプセル化され、`AnyMessage::new<T>(value: T)` と `downcast::<M>()` / `downcast_ref::<M>()` を通じて型情報を遅延取得できること。
- **FR-012**: メッセージ処理ロジックは未型付けメッセージ専用の ActorContext API を提供し、Typed レイヤーと混在させない設計ガイドラインを仕様に含めなければならない。
- **FR-013**: サンプルコードは `AnyMessage` を用いた Ping/Pong を実装し、未型付けハンドリングで性能指標を満たすことを確認しなければならない。
- **FR-014**: サンプルコードは Single Producer-Consumer の Ping/Pong シナリオを提供し、ビルド成果物が 64KB RAM 制約下で 1,000 メッセージを 1 秒以内に処理することをテストで確認できなければならない。
- **FR-015**: 拡張性確認のため、Dispatcher や Mailbox を差し替えるためのトレイト境界を公開し、外部クレートからカスタム実装を挿入できるよう仕様化しなければならない。
- **FR-016**: 全コンポーネントは循環参照を避ける設計指針（例: イミュータブル PID、弱参照テーブル）を文書化し、静的解析テストで検知できるようにしなければならない。
- **FR-017**: ActorSystem と ActorContext の API は借用を優先し、ヒープアロケーションが発生する処理には計測・再利用戦略・最大許容頻度を定義しなければならない。
- **FR-018**: Actor トレイトは `pre_start(&mut self, ctx)` / `receive(&mut self, ctx, msg)` / `post_stop(&mut self, ctx)` を提供し、`pre_start` はアクター生成直後に 1 度呼ばれ、`post_stop` は停止時に呼ばれなければならない。`receive` は `Result<(), ActorError>` を返却し、`Err(ActorError::Recoverable)` は Supervisor の再起動ロジックへ渡し、`Err(ActorError::Fatal)` は即時停止扱いとして Deadletter と EventStream で通知しなければならない。`panic!` やスタック巻き戻し不能な障害はアクターランタイムが回復を試みない旨を仕様に記載し、アプリケーションがハードリセット等の外部対処を取る前提とする。
- **FR-019**: Mailbox は System メッセージと User メッセージの優先度キューを提供し、System メッセージは常に User メッセージより先にディスパッチされなければならない。優先度判定はバックプレッシャや停止中でも維持すること。`enqueue_user` / `enqueue_system` はメッセージをキューへ追加し、ドロップ／ブロック等のポリシーに従って `SendError` を返すか、Dispatcher へのスケジューリング通知を発行するところまでを責務とし、アクターの起動やメッセージ処理を直接行ってはならない。
- **FR-020**: Mailbox は `DropNewest` / `DropOldest` / `Grow` / `Block` の 4 ポリシーを設定可能とし、初期リリースでは少なくとも `DropOldest` をデフォルトで提供し、`DropNewest` と `Grow` の正常動作を満たさなければならない。`Block` は AsyncQueue の `wait_push` など協調ブロッキング機構により実装し、ランタイムがポーリング／スピンによる忙待ちを発生させずに背圧を伝播できるインターフェイスを公開すること。
- **FR-021**: Mailbox は外部から `suspend()` / `resume()` に相当する制御を受け付け、停止期間中は User メッセージをキューに蓄積しつつ System メッセージは処理できる手段を提供しなければならない。no_std 環境と std/embassy 環境の双方で API が一貫する必要がある。
- **FR-022**: EventStream は `LogEvent` を publish できる API を提供し、少なくとも 1 つの Logger 購読者が存在してログレベル・PID・メッセージ・タイムスタンプを取得できなければならない。Logger 購読者は UART/RTT など組込み向け出力とホスト向けブリッジの双方に拡張可能であること。
- **FR-023**: ActorSystem はアクターが `spawn_child(props)` を呼び出して子アクターを生成できる API を提供し、戻り値は未型付けの `ChildRef` で管理する。親アクターは自動的に子アクターを Supervisor ツリーへ登録して監視しなければならない。親は子アクターの停止・エラー・再起動イベントを EventStream 経由で受け取れること。トップレベルの `spawn` はユーザガーディアン初期化時のみ許容され、通常フローではアクターコンテキスト経由で生成する。
- **FR-030**: ActorRef の `tell` / `ask` API は未型付けメッセージを扱うことを前提とし、ジェネリック型パラメータ経由で静的型情報を露出しない。`AnyOwnedMessage` を受け取り、背圧やサスペンドなどの送信失敗を `Result<_, SendError>` で返す。Typed レイヤーは将来の別フェーズで提供する。
- **FR-031**: ActorSystem は `user_guardian_ref()`（または同等の名前）のアクセサを提供し、ブートストラップコードはその `ActorRef` へ `Start` 等の初回メッセージを送信してアプリケーションを起動する。外部コードからの直接 `spawn` は禁止し、Context 経由の子生成のみを許可する。
- **FR-024**: ActorSystem の初期化ではユーザガーディアン（root actor）の Props を必須引数として受け取り、起動時にそのアクターを最上位コンテキストで spawn し、アプリケーションが root から子アクターを生成して機能を構築できるようにしなければならない。
- **FR-025**: アクター生成 API は任意の名前を受け付け、同一親スコープ内で一意性を保証しなければならない。名前未指定の場合は ActorSystem が衝突しない自動命名（例: `anon-{pid}`）を行い、名前から PID を逆引きできる仕組みを提供する。
- **FR-026**: MessageInvoker はメッセージをアクターへ渡す前後にミドルウェアチェーンを差し込める拡張ポイントを提供し、将来的にトレーシング・メトリクス・auth などの処理を挿入できる構造を維持しなければならない。初期リリースではチェーンは空で良いが、差し替え可能な API を公開すること。
- **FR-027**: Mailbox は Bounded/Unbounded の戦略を設定可能とし、Bounded では容量オーバ時のポリシー（FR-020）と一貫した挙動を提供し、Unbounded ではメモリ使用量を監視して EventStream/Logger へ警告を出せるメトリクスを提供しなければならない。
- **FR-032**: MailboxError / SendError は失敗した `AnyOwnedMessage` を所有（もしくは再取得可能なハンドルとして）して呼び出し元へ返し、Deadletter がそのまま保管・再配送できるようにしなければならない。Drop 系ポリシーでもメッセージを失わず、EventStream 通知に同一 payload 情報を含めること。
- **FR-033**: Mailbox の System キューはランタイム内部用の `SystemMessage`（固定スキーマ）を保持し、User キューと型で分離しなければならない。ユーザ定義メッセージが System キューへ到達しないよう型安全性を確保し、変換は ActorSystem 側で明示的に行うこと。
- **FR-034**: Mailbox と Dispatcher は no_std 組込み環境向けの同期ランナーと、std/tokio などホスト環境向けの非同期ランナーの双方へ適用できる抽象を提供しなければならない。Core レイヤーでは `async fn` を露出せず、ランナー層で `AsyncQueue` を駆動するためのフックを公開すること。
- **FR-028**: Dispatcher/MessageInvoker は 1 アクター当たりのスループット制限（例: 300 メッセージ/フェンス）を設定でき、設定値に到達した場合は制御用 System メッセージを優先しつつ残りメッセージを次ターンへ繰り越す仕組みを提供する。スループット値は Props または Mailbox 設定で構成可能とし、デフォルトは protoactor-go 相当の 300 を採用する。
- **FR-029**: ランタイムは `Context::sender()` を提供せず、応答が必要なメッセージは `reply_to: ActorRef`（もしくは同等の手段）を含むペイロード設計に従う。ActorContext は送信元を暗黙に保持しないこと。
- **FR-030**: `ask` 経路では enqueue 時に `AnyOwnedMessage` 内へ `reply_to` を保持し、MessageInvoker が処理完了後に `reply_to.tell(...)` または `ActorFuture::complete()` を呼び出す。Mailbox / Dispatcher / ActorSystem は `reply_to` を破棄せず、完了時に ActorFuture を解決するためのフックを提供しなければならない。

### 重要エンティティ（データを扱う場合）

- **ActorSystem**: PID レジストリ、Supervisor ツリー、イベント配信チャネルを保持する中心コンポーネント。
- **ActorRef / Pid**: ActorCell へのメッセージ送信に利用する軽量ハンドルと一意識別子。
- **Mailbox**: AsyncQueue に基づくメッセージバッファと処理ステータス。System キューはランタイム内部の `SystemMessage` を保持し、User キューは `AnyOwnedMessage` を保持する。
- **EventStream / Deadletter**: 監視イベントと未配達メッセージの集約ポイント。Deadletter は失敗した `AnyOwnedMessage` を完全な形で格納し、EventStream 通知も payload 情報を含む。
- **Props**: アクター生成時の構成（ファクトリ、Supervisor、Mailbox 設定）をカプセル化する定義。
- **AnyMessage**: `dyn core::any::Any` を所有せずに借用経由で参照できるメッセージコンテナ。

## 成功指標（必須）

### 定量的成果

- **SC-001**: 最小構成サンプルのアクター生成から初回メッセージ処理完了までの時間が 5ms 未満（ホスト環境）、20ms 未満（組込みターゲット）であること。
- **SC-002**: 1 秒あたり 1,000 件のメッセージ送信時に Mailbox のバックログ長が 10 件以下に収束し、Drop や Deadletter への過剰蓄積が発生しないこと。
- **SC-003**: Supervisor による再起動テストで 100 件の意図的エラーのうち 95% 以上が自動復旧し、system-wide 停止へ連鎖しないこと。
- **SC-004**: EventStream と Deadletter の監視により、未配達メッセージの検出率 100% をログ検証で確認できること。
- **SC-005**: サンプルコードにおけるメッセージ処理あたりのヒープ確保回数が 0、やむを得ず発生する場合でも 1 秒あたり 5 回未満に抑えられていることを計測で確認できること。

### 定性的成果

- **SC-006**: 開発者インタビュー（少なくとも 3 名）で「Rust no_std 環境で protoactor-go/Pekko のパターンを再利用できる」と評価されること。
- **SC-007**: ランタイムコア API が 3 つ以上の将来拡張アイデア（例: クラスタリング、永続化）への適用可能性を設計レビューで承認されること。
