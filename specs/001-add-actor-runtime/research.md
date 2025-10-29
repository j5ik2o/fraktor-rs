# Research: セルアクター no_std ランタイム初期版

## Decision: ActorError 分類ポリシー
- **Rationale**: protoactor-go の `Reason` と Pekko の `Directive` を比較し、Recoverable/Fatal の 2 区分に絞ることで Supervisor の分岐を単純化しつつ `panic!` 非介入方針と整合。Recoverable は再起動対象、Fatal は停止+通知。  
- **Alternatives considered**: protoactor-go と同様に `Restart/Resume/Stop/Escalate` の 4 区分を維持する案（no_std でのテーブル管理が複雑化）; エラーを enum ではなく trait object に委譲する案（動的ディスパッチとアロケーションが増えるため却下）。

## Decision: AsyncQueue 容量とバックプレッシャー
- **Rationale**: Ping/Pong シナリオで 1,000 msg/s を処理しつつ 64KB RAM を圧迫しないよう、Mailbox の既定容量を 64 メッセージとし、75% で送信 API に `WouldBlock` を返すバックプレッシャーを導入。`heapless::spsc::Queue` の容量計算と同等。  
- **Alternatives considered**: 無制限キュー（Alloc 増大・断片化リスク）; capacity=32（ピーク負荷時に Deadletter が増える懸念）。

## Decision: AnyMessage の借用戦略
- **Rationale**: `AnyMessage<'a>` で `&'a dyn Any` を保持し、所有権移動版 `AnyOwnedMessage` はテスト/ブリッジ用途に限定。これによりヒープ確保を避け、ダウンキャストは参照ベースで実施。  
- **Alternatives considered**: `Box<dyn Any>` による所有権転送（ヒープ確保が発生）; `enum Message` の静的多態（拡張性と未型付け方針に反する）。

## Decision: panic 非介入運用
- **Rationale**: 組込みでは panic=abort が一般的なため、ランタイムは復旧を試みず、ユーザがウォッチドッグ等でシステムリセットする運用を quickstart と API ドキュメントで案内。  
- **Alternatives considered**: panic を catch_unwind で捕捉する案（`no_std` で使用不可）; Supervisor で再起動を試みる案（スタック破壊リスク）。

## Decision: リファレンス調査結果反映
- **Rationale**: protoactor-go の `process/mailbox`、`actor/supervisor_strategy`、Pekko Classic の `EventStream` ドキュメントを主要参照とし、差分（AnyMessage, panic 方針）を data-model と quickstart で明示。  
- **Alternatives considered**: Pekko Typed を基準にする案（Typed API を導入する必要があり初期範囲を逸脱）。

## Decision: ヒープ計測アプローチ
- **Rationale**: `portable-atomic-util` のカウンタと `core::alloc::GlobalAlloc` ラッパで確保回数をトレースし、SC-005 のしきい値（≦5/秒）を quickstart で測定方法として提示。  
- **Alternatives considered**: 外部計測機材のみで確認する案（開発体験が悪化）; 試験用に `wee_alloc` を導入する案（抽象層が増え複雑化）。

## Decision: Logger 購読者の構成
- **Rationale**: Pekko の DefaultLogger と同様に EventStream 購読者としてログを扱うことで、no_std 環境でも軽量に実装でき、Deadletter/監視イベントと同じ経路で観測できる。UART/RTT への出力とホスト向けブリッジの両方に対応しやすい。  
- **Alternatives considered**: 専用 LoggerActor へ直接メッセージ送信（優先度制御が複雑化）; `tracing` のみをホスト依存で利用（no_std で利用しづらい）。

## Decision: Child actor supervision
- **Rationale**: 親アクターが `spawn_child` して Supervisor ツリーを形成するのは protoactor-go / Pekko と同様の基本機能であり、復旧ポリシーや EventStream ログに一貫性を持たせるため。Rust では Props 継承と親参照を借用ポインタで保持し、所有権循環を避ける。  
- **Alternatives considered**: ルートコンテキストのみから spawn する案（親子監視ができず Supervisor の意義が薄れる）; 子アクターをグローバル登録に切り替える案（依存が複雑化）。

## Decision: Guardian actor entry point
- **Rationale**: Pekko/Akka Typed のようにユーザガーディアン Props をエントリポイントとすることで、アプリがルートから子アクターを生成し Supervisor ツリーを自然に構成できる。RootContext のみで spawn するより構造化が容易。  
- **Alternatives considered**: 全アクターを ActorSystem から直接 spawn する案（親子関係が失われ監督ができない）。

## Decision: Actor naming registry
- **Rationale**: 親スコープごとに名前一意性を持たせ、自動命名で `anon-{pid}` を付与することで protoactor-go の ProcessRegistry と同等の UX を提供する。ログやテレメトリで名前参照が可能になる。  
- **Alternatives considered**: 名前をオプション扱いにして逆引きを提供しない案（デバッグ性低下）; グローバル一意名前（Flexibility 欠如）。

## Decision: Middleware chain abstraction
- **Rationale**: protoactor-go の Process middleware と同様にメッセージ前後で観測・フィルタを掛けられるようにしつつ、初版では空チェーンとする。将来のトレーシングや認証を挿入しやすい。  
- **Alternatives considered**: 直接 MessageInvoker にベタ書きする案（拡張性が低い）; middleware を最初から実装する案（初版で複雑化）。

## Decision: Bounded/unbounded mailbox strategy
- **Rationale**: 組込みでは Bounded を既定としつつ、ホスト検証では Unbounded を利用できるようにし、メモリ水位を EventStream で監視することで柔軟性と安全性を両立する。  
- **Alternatives considered**: 常に Bounded に固定（柔軟性不足）; Unbounded のみ提供（組込みでの OOM リスク）。

## Decision: Throughput limit per actor
- **Rationale**: protoactor-go の 300 メッセージ/アクター/ターンに倣い、過剰な占有を防ぐ。Props から構成可能にし、0 で無制限を許可。  
- **Alternatives considered**: スループット制限なし（スターべーションリスク）; 固定値で変更不可（用途に応じた調整ができない）。

## Decision: Request/Reply pattern without sender()
- **Rationale**: Akka/Pekko Typed と同様に `sender()` を廃止し、明示的に `reply_to: ActorRef` を渡すことで依存関係を明確化し、no_std での可搬性を維持する。  
- **Alternatives considered**: Classic の `sender()` を引き継ぐ案（stateful Context が複雑化し、将来 Typed レイヤー導入時に破綻する）。

## Decision: Owned message buffer for mailbox
- **Rationale**: Mailbox でメッセージを所有するため `AnyOwnedMessage` + `ArcShared` を導入し、借用型 `AnyMessage` を再構成してゼロコピーで配達できるようにする。  
- **Alternatives considered**: enqueue 毎に `Vec<u8>` へシリアライズする案（ヒープ負荷が高い）; 送信側がライフタイムを保持し続ける案（no_std で安全に扱いづらい）。

## Decision: System/User dual queue layout
- **Rationale**: 優先度制御と suspend/resume をシンプルにするため、System/User の 2 本キューに分離し、`async_mpsc` をバックエンドに採用する。  
- **Alternatives considered**: 単一キューに priority flag を持たせる案（dequeue 時の判定が複雑）; Multiqueue を自前実装する案（既存抽象を再利用できず負荷が大きい）。

## Decision: Block policy wait strategy
- **Rationale**: Busy wait を避け、WaitNode + `wait_push()` で待機させることで no_std でも適正な背圧を実現する。  
- **Alternatives considered**: spin ループで待機する案（電力・スケジューラが非効率）; Block を未実装とする案（将来の拡張性がない）。

## Decision: Ask/reply flow with ActorFuture
- **Rationale**: `reply_to: ActorRef` を AnyOwnedMessage に保持し、MessageInvoker 終了時に ActorFuture を完了させることで sender() 依存を排除しつつ ask を実現する。  
- **Alternatives considered**: Classic sender() を維持する案（Typed レイヤー導入時に矛盾）; ask をサポートしない案（ユースケースが限定される）。
