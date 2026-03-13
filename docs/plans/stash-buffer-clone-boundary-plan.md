# `stash_buffer` `Clone` 境界指摘への修正プラン

## 概要

`modules/actor/src/core/typed/stash_buffer.rs` の `contains` / `exists` / `foreach` について、今回は `M: Clone` 境界を維持する。  
理由は、現行実装が「stashed message を snapshot に clone してからユーザーコードを評価する」ことで、lock 下で `PartialEq` や callback を実行しない安全性を確保しているため。`contains` だけから `Clone` を外す変更は、この方針と両立しない。

今回のプランは `stash_buffer` のみを対象とし、`modules/actor/src/core/system/base.rs` の指摘は含めない。

## 実装変更

- `stash_buffer.rs` の `contains` / `exists` / `foreach` の doc comment を更新する。
- doc には次を明記する。
  - `Clone` 境界は意図的であること
  - clone は callback / `PartialEq` / predicate を stash 内部の lock 外で評価するための snapshot であること
  - API の目的は「軽量な参照走査」ではなく「安全な観測」であること
- コードの振る舞いは変えない。
  - `contains` のシグネチャは維持
  - `exists` / `foreach` の snapshot-clone 戦略は維持
  - `lock` 下でユーザーコードを実行する方向への変更は行わない

## テスト方針

- 既存の `stash_buffer/tests.rs` を維持し、意味がずれていないかだけ確認する。
- 追加テストを入れる場合は、実装修正ではなく意図の固定に絞る。
  - `exists` / `foreach` が callback 実行時に snapshot を使う前提で動作していること
  - `contains` が `PartialEq` ベースの観測 API として従来どおり使えること
- lock 下 callback 実行に戻っていないことを間接的に確認できる回帰テストを優先し、`Clone` 境界そのものを外すテストは追加しない。

## 受け入れ条件

- `contains` / `exists` / `foreach` の public API は現状維持
- doc を読めば `Clone` 境界の理由が分かる
- CodeRabbit 指摘に対して「問題意識は妥当だが、現行修正案では安全性を壊す」という説明ができる
- `./scripts/ci-check.sh dylint -m actor` が通る
- 最終確認として `./scripts/ci-check.sh ai all` を完走させる

## 前提・採用した判断

- このリポジトリでは後方互換性は不要だが、今回は API を狭めたまま据え置く方が安全で、無理に再設計しない
- `Clone` を外す再設計は `contains` 単体ではなく、`exists` / `foreach` を含む別タスクとして扱う
- `base.rs` 側の指摘は今回のプラン対象外
