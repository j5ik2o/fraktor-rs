## 1. 調査と PoC 方針の整理
- [ ] protoactor-go / pekko のリモート層を調査し、シリアライズ・トランスポートの抽象化パターンを比較する
- [ ] 既存 `AnyMessage` / `ActorRef` 実装を精査し、拡張ポイント（trait 化・feature gate）の影響範囲を洗い出す
- [ ] ベンチマークと障害注入シナリオの初期要件を列挙する

## 2. serializer-core / serializer-std の実装
- [ ] `modules/serializer-core` クレートを追加し、`no_std + alloc` 前提で `Serializer` トレイトとバイナリ系デフォルト実装を提供する
- [ ] `modules/serializer-std` クレートを追加し、`serializer-core` を再利用しつつ JSON など `std` 依存機能を feature で有効化する
- [ ] `AnyMessage` / `ActorSystem` 周辺に `Serializer` 登録ポイントを追加し、既定で `serializer-core` を利用する
- [ ] Cargo ワークスペースの feature 配線とドキュメントを更新する

## 3. remote-core / remote-std の実装
- [ ] `modules/remote-core` クレートを追加し、`RemotePid` 構造体・resolver API・`Transport` 抽象のインターフェースを `no_std + alloc` 前提でまとめる
- [ ] `modules/remote-std` クレートを追加し、TCP など `std` 依存トランスポート実装や再送制御を提供する
- [ ] `ActorSystem` への remote resolver 登録フローとエラー通知を実装する
- [ ] API 変更点に対するユニットテストとドキュメントを整備する

## 4. Transport PoC と段階的公開
- [ ] `remote-core` の `Transport` 抽象を使ったメッセージバッチ構造を実装する
- [ ] インメモリまたはローカル TCP の PoC を構築し、往復通信と再送挙動を検証する
- [ ] ドキュメント・ガイドを更新し、feature を内部公開 → デフォルト化するための完了条件を定義する
