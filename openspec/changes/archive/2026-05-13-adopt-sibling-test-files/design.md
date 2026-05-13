## Context

現在の unit test 規約は `#[cfg(test)] mod tests;` と nested `<module>/tests.rs` により、test を production file の外へ分離している。この方式は inline test の混入を防げる一方で、production file と同名の directory が実サブモジュール用なのか、test file だけの置き場なのか判別しづらい。

Rust では次の形にすれば、module 名を `tests` のまま維持しつつ、物理ファイルだけを sibling に移せる。

```rust
#[cfg(test)]
#[path = "hoge_test.rs"]
mod tests;
```

これにより `tests` は `hoge` の子 module のままなので、既存の `use super::*;` pattern や parent の private item へのアクセスは維持できる。物理ファイルは明確に test-only と分かる名前になり、test のためだけの `hoge/` directory は不要になる。

## Goals / Non-Goals

**Goals:**
- crate 内 unit test を sibling `*_test.rs` に置き、物理的に test-only と分かる状態にする。
- test を production file へ書かせず、Dylint による別ファイル強制を維持する。
- `tests.rs` だけを置くための `hoge/` directory を作らない。
- `#[path = ...]` の利用を、この layout に必要な test module pattern だけに限定する。
- rules、skills、lint docs、lint fixtures、source files を一貫した migration として更新する。

**Non-Goals:**
- すべての test を integration test crate へ移すこと。
- 任意の `#[path = ...]` module wiring を許可すること。
- Rust module 名を `tests` から `<module>_test` へ変えること。
- production file 内の inline `#[test]` function や inline `mod tests { ... }` 禁止を緩めること。
- no_std-sensitive crate で `tests/` へ移すべき `std::*` 依存 test logic を `src/` に残すこと。

## Decisions

### 1. `mod tests` と制約付き sibling path を使う

production file は次の形を使う。

```rust
#[cfg(test)]
#[path = "hoge_test.rs"]
mod tests;
```

module 名は `hoge_test` ではなく `tests` のままにする。これにより既存 test module は production module の子として残り、test file 内の移行差分を最小化できる。

**検討した代替案**
- `#[path = "hoge_test.rs"] mod hoge_test;`
  - module 名が変わり、test 側の差分が増えるため却下。
- `mod hoge_test;`
  - production 側に見える module 名になり、標準的な `tests` という意図が薄れるため却下。
- `<module>/tests.rs` を維持する。
  - directory ambiguity そのものが今回解決したい問題であるため却下。

### 2. `#[path = ...]` は test module wiring だけに許可する

`module-wiring-lint` は現状すべての `#[path = ...]` attribute を拒否している。この方針は維持し、任意の path wiring は拒否したまま、以下をすべて満たす場合だけ狭く例外許可する。

- item が `mod tests;` である。
- item が `#[cfg(test)]` で gate されている。
- path literal に directory separator が含まれない。
- path literal が `<declaring_file_stem>_test.rs` と一致する。
- declaring file が `hoge.rs` や `lib.rs` のような通常の Rust source file である。

例:
- `hoge.rs` は `#[path = "hoge_test.rs"] mod tests;` を許可する。
- `lib.rs` は `#[path = "lib_test.rs"] mod tests;` を許可する。
- `hoge.rs` は `#[path = "tests.rs"] mod tests;` を拒否する。
- `hoge.rs` は `#[path = "shared/hoge_test.rs"] mod tests;` を拒否する。
- `hoge.rs` は `#[path = "hoge_test.rs"] mod helper;` を拒否する。

**検討した代替案**
- `#[cfg(test)]` 配下の `#[path = ...]` をすべて許可する。
  - test layout の用途を超えて module wiring rule を弱めるため却下。

### 3. `tests-location-lint` が `_test.rs` を認識する

separate-tests lint は `*_test.rs` を test-only file として扱う必要がある。そうしないと test file を production source として読み、内部の `#[test]` function を報告してしまう。

また、`#[cfg(test)]` の直後に制約付き `#[path = "..._test.rs"] mod tests;` が続く宣言を、正しい production-side hook として扱う必要がある。対応する nested `tests.rs` layout が残っている legacy `#[cfg(test)] mod tests;` は migration target とする。

**検討した代替案**
- 旧 layout と新 layout の両方を無期限に受け入れる。
  - release 前の repository で legacy compatibility path を残さない方針のため却下。

### 4. 関連 lint の ignore logic を更新する

`type-per-file-lint` と `ambiguous-suffix-lint` はすでに `tests.rs`、`*_tests.rs`、`tests/` directory を無視している。test helper type や test helper name が production type / production name rule で判定されないよう、`*_test.rs` も同様に無視する。

**検討した代替案**
- test helper にも production lint rule をすべて適用する。
  - test file では fixture や helper の形が production より緩くなることがあり、production と同じ制約を課すと過剰設計になりやすいため却下。

### 5. no_std-sensitive な std 依存 test は `src/` 外に置く

この change は crate 内 unit test のファイル形状だけを変える。no_std-sensitive crate の std 依存 test logic は、安全に `src/` に残せない場合 `tests/` または分離された fixture へ置く、という既存 rule は上書きしない。

## Risks / Trade-offs

- [新 layout は `#[path = ...]` に依存する] → 例外条件を厳密にし、許可例・拒否例の両方を Dylint UI fixture で検証する。
- [migration が多くの file に触れる] → crate / module batch ごとに移行し、各 batch 後に targeted test を実行する。
- [root module test には専用規約が必要] → `lib.rs` には `lib_test.rs` を使い、rules と lint tests に明記する。
- [`*_test.rs` が production-oriented lint の ignore list から漏れる] → 現在 `tests.rs` または `*_tests.rs` を特別扱いしている lint をすべて更新する。
- [no_std source tree rule が `_test.rs` なら std test を許すと誤読される] → std 依存 no_std test は引き続き `tests/` へ移すことを rules / specs に明記する。

## Migration Plan

1. rules と skill references を `<module>/tests.rs` から `<module>_test.rs` へ更新する。
2. Dylint implementation と UI fixture を更新する。
   - `tests-location-lint`
   - `module-wiring-lint`
   - `type-per-file-lint`
   - `ambiguous-suffix-lint`
3. 既存の `src/**/tests.rs` を移行する。
   - `hoge/tests.rs` → `hoge_test.rs`
   - parent `hoge.rs`: `#[cfg(test)] mod tests;` → `#[cfg(test)] #[path = "hoge_test.rs"] mod tests;`
   - root `src/tests.rs` → `src/lib_test.rs` に移し、`lib.rs` を更新する。
4. batch ごとに targeted Dylint check と crate test を実行する。
5. 最終的に `./scripts/ci-check.sh ai all` を実行する。
