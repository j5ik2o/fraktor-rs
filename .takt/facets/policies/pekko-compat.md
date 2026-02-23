# Pekko互換性レビューポリシー

## REJECT判定基準

| 問題 | 判定 |
|------|------|
| Pekko APIに対応するメソッドが欠落 | タスク指示に含まれるならREJECT |
| 型パラメータの対応が不正確 | REJECT |
| no_std互換でない実装がcoreに配置 | REJECT |
| `&self`/`&mut self` の使い分けがCQS原則に違反 | REJECT |
| 禁止サフィックス（Manager, Service等）の使用 | REJECT |
| テストが欠落 | REJECT |
| 参照実装を読まずに「互換」と主張 | REJECT |
| YAGNI違反（タスク範囲外の機能追加） | REJECT |
