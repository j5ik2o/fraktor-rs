## Context

step03〜step05 を経て `actor-core/test-support` feature は **完全に空 (`[]`)** になり、`actor-core/src/` 配下に `feature = "test-support"` を参照する `#[cfg(...)]` は **0 件** になった (step05 完了状態)。本 change は最終ステップとして、空のままになっている feature 定義そのものと、各クレートに残る関連参照を機械的に削除する。

### 全数棚卸し (step06 開始時点)

| カテゴリ | 場所 | 件数 | 内容 |
|---|---|---|---|
| ① actor-core feature 定義 | `modules/actor-core/Cargo.toml:19` | 1 | `test-support = []` |
| ② actor-core `[[test]] required-features` | `modules/actor-core/Cargo.toml` | 8 | `required-features = ["test-support"]` × 8 セクション |
| ③ actor-core dev-dep adaptor-std features | `modules/actor-core/Cargo.toml:43` | 1 | `fraktor-actor-adaptor-std-rs = { ..., features = ["test-support"] }` (これは actor-adaptor-std の test-support を有効化、step06 後も残る) |
| ④ 下流 crate の test-support feature 定義（actor-core 経由 forward） | actor-adaptor-std (line 17)、cluster-core (17)、cluster-adaptor-std (18)、remote-adaptor-std (17) | 4 | `["fraktor-actor-core-rs/test-support"]` を含む定義 |
| ⑤ 下流 dev-dep の `actor-core features=["test-support"]` | 8 ファイル | 8 | actor-adaptor-std (37)、cluster-core (30)、cluster-adaptor-std (36)、persistence-core (29)、remote-adaptor-std (34)、stream-core (29)、stream-adaptor-std (26)、showcases/std (9) |
| ⑥ actor-core/src の `feature = "test-support"` 参照 | `modules/actor-core/src/**/*.rs` | 0 | step05 完了済み |

### 重要な制約

- **actor-adaptor-std/test-support feature は残す**: actor-adaptor-std の `test-support` feature は `TestTickDriver`、`new_empty_actor_system*` の **公開ゲート** として使われている (step03 で確立)。actor-core/test-support とは独立した責務を持つため、本 change では actor-adaptor-std/test-support を削除しない。ただし `["fraktor-actor-core-rs/test-support"]` の forward 部分は撤去する（actor-core 側で消えるため）
- **同様に cluster-core/test-support、remote-adaptor-std/test-support、cluster-adaptor-std/test-support も残す**: それぞれ独自の用途を持つ可能性がある（caller を本 change では再評価しない）
- **dev-dep `fraktor-actor-core-rs = { workspace = true, features = ["test-support"] }` の取り扱い**: feature 削除後、`fraktor-actor-core-rs = { workspace = true }` 単独になる。これは prod dep (各 crate の `[dependencies]`) と完全に同じ宣言になり redundant。Cargo は許容するが警告/混乱の元なので **dev-dep 行ごと削除**するのが望ましい (Decision 3)

## Goals / Non-Goals

**Goals:**

- `modules/actor-core/Cargo.toml` から `test-support` feature 定義と関連 `required-features` を全廃
- 全下流 crate の `Cargo.toml` から `fraktor-actor-core-rs/test-support` への参照を全廃
- 下流 crate の dev-dep が prod dep と同等になった場合、redundant な dev-dep 行を削除
- workspace 全体ビルドおよびテストが、feature 削除前と同じ pass 率を維持
- `actor-test-driver-placement` capability に「`actor-core` には `test-support` feature が存在しない」を検証する Scenario を追加 (MODIFIED)

**Non-Goals:**

- `actor-adaptor-std/test-support`、`cluster-core/test-support`、`cluster-adaptor-std/test-support`、`remote-adaptor-std/test-support` feature の削除や見直し（独自責務、別 change）
- `actor-core` の `alloc` / `alloc-metrics` feature の見直し（別スコープ）
- `actor-core` 内部に残存する `pub(crate)` 限定の test-only ヘルパ (step03 dev-cycle workaround 由来) の削除（別 change）
- portable-atomic 関連の整理 (step07 / step08 で扱う)

## Decisions

### Decision 1: actor-core/Cargo.toml の機械的削除

```toml
# Before (lines 15-19)
[features]
default = []
alloc = []
alloc-metrics = []
test-support = []

# After
[features]
default = []
alloc = []
alloc-metrics = []
```

`[[test]]` セクション 8 個の `required-features = ["test-support"]` 行を削除（行のみ削除、`[[test]]` 本体は残す）。

### Decision 2: 下流 crate の test-support feature 定義の更新

actor-core/test-support を forward していた 4 件:

- `actor-adaptor-std/Cargo.toml:17` `test-support = ["fraktor-actor-core-rs/test-support"]` → `test-support = []`
- `cluster-core/Cargo.toml:17` `test-support = ["fraktor-actor-core-rs/test-support"]` → `test-support = []`
- `cluster-adaptor-std/Cargo.toml:18` `test-support = ["fraktor-cluster-core-rs/test-support", "fraktor-actor-core-rs/test-support"]` → `test-support = ["fraktor-cluster-core-rs/test-support"]`
- `remote-adaptor-std/Cargo.toml:17` `test-support = ["fraktor-actor-core-rs/test-support"]` → `test-support = []`

cluster-core / remote-adaptor-std / actor-adaptor-std の test-support feature が空 `[]` になっても、独自の用途 (capability gate 等) があれば feature 自体は維持する (本 change スコープ外)。

### Decision 3: 下流 dev-dep `actor-core features=["test-support"]` の取り扱い

各 crate Cargo.toml の dev-dep 行:

```toml
fraktor-actor-core-rs = { workspace = true, features = ["test-support"] }
```

から `features = ["test-support"]` を削除すると `{ workspace = true }` になる。これは同一 crate の `[dependencies]` セクション（prod dep）と完全一致するため **dev-dep 行ごと削除する**。

例外: `showcases/std/Cargo.toml:9` は `[dependencies]` (prod) に書かれているため、行ごと削除ではなく `features = ["test-support"]` のみ削除する:

```toml
# Before
fraktor-actor-core-rs = { workspace = true, features = ["test-support"] }
# After
fraktor-actor-core-rs = { workspace = true }
```

### Decision 4: actor-core の dev-dep `actor-adaptor-std features=["test-support"]` は残す

`modules/actor-core/Cargo.toml:43` の以下は **そのまま残す**:

```toml
fraktor-actor-adaptor-std-rs = { workspace = true, features = ["test-support"] }
```

これは actor-adaptor-std の test-support feature を有効化するもので、actor-adaptor-std 側の `TestTickDriver`、`new_empty_actor_system*` を actor-core の integration test (`tests/*.rs`) で利用するために必須。step03 で確立した dev-cycle 経路。

### Decision 5: spec delta は MODIFIED でカバー

提案で挙げた 2 案のうち **案 A** (既存 capability に Scenario 追加) を採用。

`actor-test-driver-placement` capability の Requirement「actor-core では feature ゲート経由で内部 API の可視性を拡大してはならない」(step05 で追加) に Scenario を追加 (MODIFIED):

- `actor-core/Cargo.toml` の `[features]` セクションに `test-support` が存在しないことを検証
- 全下流 crate の actor-core dev-dep に `features = ["test-support"]` が含まれないことを検証

これにより「step06 後に test-support 関連参照が残らない」というルールが spec で機械的に検査可能になる。

## Risks / Trade-offs

- **[Risk] 下流 crate の dev-dep 行削除で別の隠れた依存が壊れる**:
  - Mitigation: 各 crate を `cargo test` で個別検証する。redundant 削除前後でテスト結果に差が出ないことを確認
- **[Risk] actor-adaptor-std / cluster-core / remote-adaptor-std の test-support 定義を空 `[]` にした後、別の forward が必要になる**:
  - Mitigation: 本 change は最小差分。空にした後で問題が出れば該当 crate 側で個別対応
- **[Trade-off] actor-adaptor-std 等の test-support feature 自体を残すか削除するかの判断**:
  - 受容: 本 change は actor-core/test-support のみがスコープ。他クレートの feature 整理は独立した責務なので将来の change で扱う
- **[Risk] `[[test]] required-features` 削除後、実体は同じだが Cargo の解釈が変わる**:
  - 確認: required-features 削除しても、その test は常に build/run される（feature 不要のため）。step05 後 actor-core/test-support が `[]` だったので required-features があってもなくても挙動は同じ
- **[Risk] `cargo test --workspace --features test-support` のような呼び出しが CI/ローカルで使われている**:
  - Mitigation: 削除後は「unknown feature `test-support` for actor-core」エラーになるため、検出は容易。既知の呼び出し箇所 (`scripts/ci-check.sh` 等) を grep で確認

## Migration Plan

1. **Phase 1: 下流 crate の参照削除** (順序依存なし)
   - 8 ファイルから dev-dep `fraktor-actor-core-rs = { ..., features = ["test-support"] }` 削除（Decision 3）
   - 4 ファイルから `test-support` feature 定義の forward を削除（Decision 2）
   - 各 crate ごとに `cargo test -p <crate>` で pass 確認
2. **Phase 2: actor-core 本体の削除**
   - `modules/actor-core/Cargo.toml` の `test-support = []` 行を削除
   - 8 個の `[[test]] required-features = ["test-support"]` 行を削除
   - `cargo test -p fraktor-actor-core-rs` で pass 確認
3. **Phase 3: workspace 全体検証**
   - `cargo test --workspace` 全 pass
   - `cargo build --workspace --no-default-features` pass
   - `cargo build --workspace --all-features` pass
   - `Grep "test-support" modules/actor-core/Cargo.toml` で 0 件
   - `Grep 'fraktor-actor-core-rs.*test-support' modules/ showcases/` で 0 件
   - `./scripts/ci-check.sh dylint` pass
   - `./scripts/ci-check.sh ai all` pass
4. **Phase 4: spec delta 適用**
   - `actor-test-driver-placement` capability に MODIFIED で Scenario 追加
   - `openspec validate --strict` pass
5. **Phase 5: ドキュメント更新**
   - `docs/plan/2026-04-21-actor-core-critical-section-followups.md` 残課題 1 全体を「解消済み」に更新（責務 A/B/C/feature 削除すべて完了）
6. **Phase 6: コミット + PR**
   - 論理単位での commit (Phase 1 を crate ごとに分けるか、一括にするかは判断)
   - PR 作成 → レビュー → マージ
7. **Phase 7: archive**

ロールバックは git revert で完結する。

## Open Questions

- 下流 crate (actor-adaptor-std / cluster-core / cluster-adaptor-std / remote-adaptor-std) の test-support feature が空 `[]` になった場合、それ自体を削除するかは別 change で再評価する。本 change ではスコープ外として残す
- showcases/std の `fraktor-actor-core-rs` を `[dependencies]` に持つ設計が妥当かは別問題。本 change では `features = ["test-support"]` のみ除去
- step07 (portable-atomic 評価) との順序は独立。step06 マージ後、step07 に進める
