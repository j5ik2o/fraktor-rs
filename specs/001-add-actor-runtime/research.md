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
