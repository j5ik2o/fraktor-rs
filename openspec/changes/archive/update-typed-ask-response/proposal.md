# 目的
TypedActorRefGeneric::ask で得られる応答を compile-time で型保証するため、AskResponseGeneric の typed 版 (`TypedAskResponseGeneric<R, TB>`) を導入し、呼び出し側が downcast なしで `R` を扱えるようにする。

# 背景
現在の typed ask は内部的に untyped な `AskResponseGeneric` を返しており、返信 future から値を取り出す際は `AnyMessageGeneric` からの downcast が必須になっている。結果として typed API であっても実際には `R` の安全な受け取りを手作業で保証する必要があり、バグの温床になっている。

# スコープ
- `TypedAskResponseGeneric<R, TB>` 型の設計と公開
- `TypedActorRefGeneric::ask` から typed 応答を取得できる仕組み
- 既存の `AskResponseGeneric` との相互変換・互換性維持
- テストおよび example で typed ask の利用方法を示す

# 非スコープ
- ActorFuture の別実装や scheduler の変更
- ask パターン自体のプロトコル変更（タイムアウト、キャンセル等）

# 成功基準
- 呼び出し側が追加の downcast/unwrap なしに `R` を取得できる
- typed ask API の単体テストと example が存在し CI で実行される
- 既存の untyped ask API が後方互換を維持する
