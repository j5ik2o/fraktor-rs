## Context

`actor-core/test-support` feature は前回 change（`drop-actor-core-critical-section-dep`、PR #1605/#1606）以前から、`critical-section/std` impl provider をダウンストリームに自動配給する役割（責務 A）を担ってきた。前回 change で Cargo features 構文制約に対応するため `test-support = ["dep:critical-section", "critical-section/std"]` 形式に整理されたが、責務 A 自体は `actor-core` に残ったままだった。

`critical-section` クレートの本来の作法では、ライブラリは依存を宣言するのみで impl 選択はバイナリ（test バイナリ、showcase バイナリ等）の責任とされる。本 change は `actor-core` が責務 A を抱え続ける理由がないこと（前回 change で tick_feed.rs から直接 use が消え、impl 選択の機能性のみが残っている）を踏まえ、責務 A を **本来の所在地（バイナリ側）に移譲** する。

実態調査（前回 change の hand-off メモおよび本 change の追加調査）により以下が判明:

- `modules/actor-adaptor-std/Cargo.toml:37-38` および `modules/persistence-core/Cargo.toml:28-29` は **既に dev-deps で `critical-section = { features = ["std"] }` を直接記述済み**。これは `actor-adaptor-std/Cargo.toml:30-36` のコメントによれば「integration tests と benches のシンボル解決失敗を防ぐため」の意図的な二重宣言。本 change で `actor-core/test-support` から impl provider 関連が消えれば、二重宣言は単純な「自前で取得」に整理される
- 他 5 クレート（cluster-core、cluster-adaptor-std、remote-adaptor-std、stream-core、stream-adaptor-std）は `actor-core/test-support` 経由のみで impl provider を取得
- `showcases/std/Cargo.toml:9-10, 19-20` は `[dependencies]` で `actor-core/test-support` および他 3 クレートの `test-support` を有効化
- `remote-core/Cargo.toml:17` の `test-support = []` は完全に空、利用箇所なし
- `actor-core/Cargo.toml:51-86` の 8 個の `[[test]]` ブロックは `required-features = ["test-support"]`。本 change では `test-support` feature 自体は残すため影響なし

## Goals / Non-Goals

**Goals:**
- `actor-core` から `critical-section` クレートへの **direct dependency edge を完全削除** する（`optional = true` 含めて Cargo.toml の `[dependencies]` から消す）
- `test-support` feature を `[]` に簡略化し、impl provider 関連責務を撤去する
- 各バイナリ単位（test/bench/showcase）で `critical-section = { features = ["std"] }` を `[dev-dependencies]` または `[dependencies]` に直接記述する形に統一する
- `actor-lock-construction-governance` Requirement B の例外条項を削除し、規約をシンプル化する
- 全 9 クレートのビルド・テストが従来どおり通る

**Non-Goals:**
- `test-support` feature 自体の完全退役（責務 B/C のため残す）
- 責務 B（`TestTickDriver`、`new_empty` 等の API 公開）の分離・移動
- 責務 C（内部 API の `pub` 格上げ）の分離
- `portable-atomic/critical-section` feature の撤去
- `utils-core/Cargo.toml:26` の `critical-section` 直接依存の見直し（utils-core は backend 実装層として例外許可）
- `actor-core/Cargo.toml:36` の `spin` 直接依存の撤去

## Decisions

### Decision 1: 責務 A を完全に各バイナリ側に移譲する

**選択**: `actor-core/test-support` から `critical-section/std` 関連を完全削除し、各バイナリが自前で `critical-section = { features = ["std"] }` を宣言する。

**根拠**:
- `critical-section` クレートの標準作法（ライブラリは依存宣言のみ、impl 選択はバイナリ側）に準拠する
- `actor-adaptor-std`・`persistence-core` で意図的に行われていた二重宣言（context 参照）が「単純な自前取得」に整理される
- `test-support` feature の名称と機能が一致するようになる（impl provider 提供は名前から類推されない責務だった）

**代替案と却下理由**:
- 案 A: 専用 feature `std-impl` を新設して責務 A を切り出す → feature 数が増え、ダウンストリームが「`test-support` か `std-impl` か」を判断する必要が出る。標準作法に従えば feature 自体が不要
- 案 B: `actor-core` の `test-support` に責務 A を残し続ける → 現状維持。`test-support` の責務混在が解消しない

### Decision 2: `actor-core/Cargo.toml` から `critical-section` を完全削除する

**選択**: `actor-core/Cargo.toml:24` の `critical-section = { workspace = true, default-features = false, optional = true }` 行を完全に削除する。

**根拠**:
- 責務 A 撤去後、`actor-core` 自身が `critical-section` を直接依存する理由が完全に消える
- `[dev-dependencies]` の `critical-section = { workspace = true, features = ["std"] }`（:42）は維持（actor-core 自身の `cargo test` で必要）
- ソースコードからの use は前回 change で既に消えている
- `portable-atomic` 経由の transitive 依存は引き続き存在するが、これは `[dependencies]` 直接宣言ではない

**代替案と却下理由**:
- 案 A: `optional = true` のまま残す → 利用されない依存宣言を残すのは死に依存。Requirement B の規約簡素化の意義も失われる

### Decision 3: 各バイナリ側の修正方針は「最小追加・既存非破壊」

**選択**: ダウンストリームクレートの修正は以下に統一する:

1. `[dev-dependencies]`（または `[dependencies]`）に `critical-section = { workspace = true, features = ["std"] }` を 1 行追加
2. 既存の `fraktor-actor-core-rs = { features = ["test-support"] }` 等の記述は維持（責務 B/C のため引き続き必要）
3. `actor-adaptor-std` と `persistence-core` は既に直接記述済みのため変更不要

**根拠**:
- `test-support` feature 自体の退役は別 change のため、`test-support` 利用箇所はそのまま
- 各クレートで responsibility A の取得経路を明示することで「誰が impl provider を要求しているか」が明確になる
- 1 行追加で済む最小変更

**代替案と却下理由**:
- 案 A: workspace ルートの `Cargo.toml` の `[workspace.dependencies]` に `critical-section = { features = ["std"] }` をデフォルト有効化として宣言 → workspace 全体に影響、no_std 利用者にも `std` feature が誤って入る危険

### Decision 4: `actor-lock-construction-governance` Requirement B の例外条項を削除する

**選択**: 前回 change で追加した「`optional = true` かつ feature gated な impl provider 用エントリは例外」の例外条項を MODIFIED Requirements で削除する。

**根拠**:
- 本 change で例外を発動する箇所（`actor-core` の `critical-section` optional 宣言）が消える
- 例外を残すと「将来再び optional 宣言を作る余地」を spec として認めることになり、本 change の方針（バイナリ側責任）に反する
- spec はシンプルに「`actor-*` の `Cargo.toml` は primitive lock crate を `[dependencies]` に直接宣言してはならない（推移的依存のみ許可）」と整理する

**代替案と却下理由**:
- 案 A: 例外条項を残しつつ「現時点で発動箇所なし」と注記 → 将来の逸脱の余地を残す。シンプル化の意義が損なわれる

### Decision 5: `remote-core` の幽霊定義を削除する

**選択**: `modules/remote-core/Cargo.toml:17` の `test-support = []` を削除する。

**根拠**:
- 完全に空、`remote-core/src` 内に `cfg(any(test, feature = "test-support"))` は 0 件（前回 change の調査で確認済み）
- 以前は `remote-adaptor-std` の dev-deps 記述のためだけに存在していたが、本 change で `remote-adaptor-std` 側の `fraktor-remote-core-rs/test-support` 記述も整理可能（test-support 自体は責務 B/C のため残るが、remote-core には責務 B/C が無いため不要）

**代替案と却下理由**:
- 案 A: 幽霊定義を残す → 死に feature 定義を spec / コードに残す意味なし

### Decision 6: showcases/std は `[dependencies]` に直接記述する

**選択**: `showcases/std/Cargo.toml` の `[dependencies]` に `critical-section = { workspace = true, features = ["std"] }` を追加する（`[dev-dependencies]` ではなく）。

**根拠**:
- `showcases/std` は実行バイナリとして `cargo run` されるため、`[dependencies]` で常時必要
- 既に `actor-core/test-support` を `[dependencies]` で常時有効化していた事実と整合（前回 change の調査で「test-support の常時有効化が命名と実態の乖離」と指摘した部分が、本修正で正される）

**代替案と却下理由**:
- 案 A: `showcases/std` を crate 構造変更し、impl provider 取得を別経路にする → 過剰

## Risks / Trade-offs

- **[Risk] 各クレートの `critical-section/std` 追加忘れ** → Mitigation: tasks フェーズで対象 5 クレート + showcase を網羅的に修正、各クレートで `cargo test` または `cargo build` を確認
- **[Risk] `[[test]] required-features = ["test-support"]` の挙動変化** → Mitigation: `actor-core/Cargo.toml` の `test-support = []` 化後も、test-support 有効化時に `[dev-dependencies]` の `critical-section/std` が引き込まれるため、actor-core 自身のテストは従来どおり動く（Decision 2 で言及）
- **[Risk] 移行漏れによる no_std ターゲットへの影響** → Mitigation: `actor-core` の `[dependencies]` から `critical-section` を完全削除しても no_std ターゲットには元々 `critical-section/std` は無関係。ダウンストリームの dev-deps 経由は test/bench でのみ有効化されるため no_std ビルドに影響なし
- **[Trade-off] ダウンストリームに 1 行のボイラープレートが増える** → 受容: 標準作法に従う代償として妥当。長期的には feature 構造のシンプル化メリットが上回る
- **[Trade-off] `test-support` feature が中途半端な状態（責務 B/C のみ）になる** → 受容: 責務 B/C の退役は別 change でスケジュール済み。本 change は段階的退役の第 1 ステップ

## Migration Plan

本 change はビルド設定変更が主体であり、ダウンストリーム移行手順は不要。

1. **Phase 1**: ダウンストリーム 5 クレートの dev-dependencies に `critical-section = { features = ["std"] }` 追加
2. **Phase 2**: `showcases/std/Cargo.toml` の `[dependencies]` に `critical-section = { features = ["std"] }` 追加
3. **Phase 3**: 各クレートで `cargo test` / `cargo build` 確認
4. **Phase 4**: `actor-core/Cargo.toml` の `test-support = []` 化、`[dependencies]` から `critical-section` 削除
5. **Phase 5**: `remote-core/Cargo.toml:17` の幽霊定義削除（連動して `remote-adaptor-std` の test-support 配列も整理）
6. **Phase 6**: `openspec validate --strict` で artifact 整合確認（spec delta は本 change の `specs/actor-lock-construction-governance/spec.md` に記述済み、`openspec apply` 時に main spec へ自動 merge）
7. **Phase 7**: `./scripts/ci-check.sh ai all` 実行確認

ロールバックは git revert で完結する（外部状態への影響なし）。

## Open Questions

- なし（必要な設計判断は本 design で確定済み）
