# add-typed-actor-layer

## 背景
protoactor-go や Pekko Typed では、メッセージ型とライフサイクル制御を静的に記述できるレイヤーがあり、利用者は untyped メールボックスや低レベル API を直接扱わない。cellactor-rs ではまだ Typed 相当の境界が存在せず、アクター生成・監視戦略・Behavior 切り替えを利用者が都度組み立てる必要がある。

## 目的
- 型安全なアクター作成・メッセージ送受信 API を提供し、誤ったメッセージ配送をコンパイル時に防ぐ。
- Supervisor/Behavior/Context の標準的なモデルを規定し、pekko/protoactor と同等の開発体験を Rust で実現する。
- 将来の async fn ベース記述や remote transport に対応できる安定した抽象レイヤーを用意する。

## スコープ
- Typed Actor System（ガーディアン、Spawning API、ActorRef/ActorContext 等）の仕様策定。
- Behavior 定義の状態遷移、OneForOne/AllForOne などの監視戦略フックの要求事項。
- メッセージ型と Command/Event の区別、Cluster/Remote 連携を見据えた Envelope 契約。

## 非スコープ
- 実装詳細（executor 選択、mailbox 実装、async fn アダプタ）。
- Remote 通信プロトコルやシリアライザの仕様化。

## リスク・懸念
- ランタイム最適化への制約: Typed レイヤーにより内部表現の自由度が下がる可能性。
- 監視戦略の柔軟性: pekko 互換を重視しすぎると Rust イディオムと衝突しうる。

## 計画
1. Typed レイヤーの要件とシナリオを spec として定義する。
2. ActorRef/Behavior/ActorContext の API 境界を Rust 型として設計し、protoactor-go/pekko を参照。
3. Supervisor 戦略およびガーディアンのフローを文書化する。
4. 将来の async fn アダプタを受け入れるための Hook/Adapter 要件を整理する。
