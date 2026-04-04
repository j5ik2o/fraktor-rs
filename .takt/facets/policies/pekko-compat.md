# Pekko互換性レビューポリシー

Pekko参照実装とfraktor-rs実装のAPI対応関係の正確性と完全性を保証する。

## 優先順位

判断順序は以下に固定する。

1. Rust / fraktor-rs の設計原則
2. 利用者が観測する振る舞い契約
3. API surface の対応
4. 内部構造の対応

Pekko に似ていても、Rust として不自然な設計は採用しない。

## 原則

| 条件 | 判定 |
|------|------|
| Pekko APIに対応するメソッドが欠落（タスク指示に含まれる場合） | REJECT |
| 型パラメータの対応が不正確 | REJECT |
| no_std互換でない実装がcoreに配置 | REJECT |
| `&self`/`&mut self` の使い分けがCQS原則に違反 | REJECT |
| 禁止サフィックス（Manager, Service等）の使用 | REJECT |
| テストが欠落 | REJECT |
| 参照実装を読まずに「互換」と主張 | REJECT |
| YAGNI違反（タスク範囲外の機能追加） | REJECT |
| wrapper / alias だけで互換面を偽装している | REJECT |
| `ignore()` / `empty()` / `self` 返却だけの fallback を public API に露出している | REJECT |
| no-op / placeholder のまま Pekko互換名を public にしている | REJECT |
| 上記REJECT条件に該当せず、タスク指示の要件を満たしている | APPROVE |

## 禁止事項

- レビューアはコードを自ら修正してはならない（指摘と修正案の提示のみ）
- 参照実装を確認せずに互換性を判定してはならない
- 「似た名前がある」ことをもって互換と判定してはならない
- Rust で不自然な公開 API を、Pekko と同じ見た目に寄せるためだけに導入してはならない
