# 目的
pekko/akka typed の Behavior/Behaviors API と同等の状態遷移モデルをRustでも提供し、TypedActor インターフェイスに依存せずに関数合成的なビルダーでガーディアンや子アクターを記述できるようにする。

# 背景
現在の typed レイヤーは `TypedActor` trait 実装と Props 生成のみに対応しており、pekko が提供している `Behaviors.receiveMessage` 等の関数型 API が存在しない。そのためシンプルなアクターの記述でも新規 struct と impl を明示的に書く必要があり、また `Behavior::same` などの状態遷移語彙も欠如している。

# スコープ
- Behavior trait と Behaviors ビルダー群を追加し、`same`/`stopped`/`ignore`/`receiveMessage`/`receiveSignal` を最低限カバーする
- ハンドラから返却された Behavior へ状態遷移できるようにする
- Behavior を TypedProps 経由でランタイムへ渡せるアダプタを実装する
- 単体テストと examples で代表的な遷移（特に receiveMessage -> 新 Behavior）の動作を確認する

# 非スコープ
- pekko が提供する withTimers / supervise / setup 等の拡張 DSL
- sharded actor などクラスタ依存の高度な機能

# 成功基準
- Behaviors::receiveMessage で作成した挙動が同じファイル内で `same`, `stopped`, `ignore`, あるいは別の Behavior を返し、実際に次のメッセージでその挙動が使用される
- 追加した API を利用した単体テストと example が成功する
