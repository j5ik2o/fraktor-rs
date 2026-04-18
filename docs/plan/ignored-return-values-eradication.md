# 戻り値握りつぶしの撲滅計画

## 1. 目的と背景

`let _ = <Result/Option/#[must_use]>;` および `.ok();` 等による戻り値の握りつぶしを、
プロジェクトのコードベースから全面的に解消する。
同時に、再発を機械的に防止する CI 強制を追加し、
ルール `.agents/rules/ignored-return-values.md` への完全準拠を保証する。

### ユーザー要求

> 戻り値を無視するコードが大量にある。`let _ = ...` など。
> 品質保証の観点でこの問題を対処してください。戻り値を無視するのはもってのほかです。

「品質保証の観点で対処」という語から、以下 2 点を暗黙要求として導出する（要件 5,6）。

- 既存違反の全件修正（再発防止なき修正は品質保証にならない）
- 再発を機械的に防ぐ CI 強制（修正だけでは将来混入を防げない）

## 2. 参照資料

唯一のソース・オブ・トゥルース（本計画で参照する根拠）は以下。

- **ルール**: `.agents/rules/ignored-return-values.md`
  - MUST: `Result` は `?` / `match` / `if let Err(...)` のいずれかで明示的に扱う
  - MUST NOT: `let _ = expr;` で Result/#[must_use] を捨てる、`.ok();` でエラー情報を捨てる、`match _ { Ok(_) => {}, Err(_) => {} }` のように無言で両方捨てる
  - 許容例外（直前コメント必須）: ①Drop / shutdown best-effort で回復不能かつ整合性影響なし ②補助メトリクス ③`Vec::pop` / `HashMap::remove` 相当 ④`Arc::into_raw` / `from_raw` 相当の低レベル所有権操作
  - 機械的強制指針: `clippy::let_underscore_must_use` と `unused_must_use` を CI で failure 扱い
- **命名規約**: `.agents/rules/rust/naming-conventions.md`（新 lint 名の曖昧サフィックスチェック用）
- **既存 Dylint**: `lints/ambiguous-suffix-lint/` をテンプレートとする
- **CI 配線**: `scripts/ci-check.sh` L601-665（`lint_entries` ハードコード配列）

## 3. 現状分析

### 3.1 既存設定

ルート `Cargo.toml` L60-66:

```toml
[workspace.lints.rust]
unused_must_use = "deny"
unexpected_cfgs = { level = "warn", check-cfg = ['cfg(fraktor_disable_tell)'] }

[workspace.lints.clippy]
let_underscore_must_use = "deny"
```

両 lint は既に `deny` 設定済み。`#[allow(...)]` による抑制も 0 件。

### 3.2 実在する違反件数

- `modules/**` 全体で `let _ = ...;` が **549 箇所 / 150 ファイル**
- `.ok();` 式文が **11 箇所 / 6 ファイル**

設定済みの既存 2 lint では検出されていない。原因は以下 5 パターンを取りこぼすこと。

| ID | パターン | 既存 lint で検出不可の理由 |
|----|----------|-------------------------|
| P1 | `#[must_use]` 属性が付かない Result 風型・Handle 型の破棄（例: `oneshot::Sender::send(())`, `tokio::JoinHandle`） | 型自体が `#[must_use]` 付与されていないと `let_underscore_must_use` は発火しない |
| P2 | `x.ok();`（`Result → Option` 変換で `must_use` 鎖が切れる） | `.ok()` の戻り値 `Option` は実装上 `#[must_use]` でも、式文はそのまま型エラーにならない |
| P3 | default trait impl の `let _ = (key, now);` 引数破棄 | 引数 drop は `let_underscore_must_use` 対象外 |
| P4 | `let _ = local_value;` でのライフタイム延長 | 右辺が RAII ガードなら `let_underscore_lock` などで拾える可能性はあるが未設定 |
| P5 | `let _ = Box::from_raw(ptr);` などの明示 drop | `Box` は `#[must_use]` ではないため発火しない |

### 3.3 カテゴリ別サンプル

実サンプルを読んで分類を確認済み。

- **A（純粋な違反 / Result 握りつぶし）**:
  - `modules/cluster-adaptor-std/src/std/tokio_gossiper.rs:97,99,100`（`oneshot::Sender::send`, `runtime.spawn`, `JoinHandle.await`）
  - `modules/actor-adaptor-std/src/std/tick_driver/tokio_tick_driver.rs:119-122`
  - `modules/actor-core/src/core/kernel/actor/scheduler/scheduler_core.rs:242,351,445`
  - `modules/cluster-core/src/core/pub_sub/batching_producer_generic.rs:158,162`
  - `modules/cluster-core/src/core/pub_sub/cluster_pub_sub/cluster_pub_sub_impl.rs:172,333`
  - `modules/cluster-core/src/core/cluster_core.rs:455`
  - `modules/cluster-core/src/core/pub_sub/pub_sub_broker.rs:145`
  - `modules/remote-core/src/association/base.rs:216`
  - `modules/actor-core/src/core/kernel/dispatch/mailbox/mailbox_queue_state.rs:64,68`
  - `modules/actor-core/tests/system_events.rs:49`
  - `modules/actor-core/tests/death_watch.rs:37,197`
  - `modules/cluster-core/src/core/cluster_core/tests.rs:1056`
- **B（trait default impl の引数破棄）**:
  - `modules/cluster-core/src/core/identity/identity_lookup.rs:49,59,71,82,92`
  - `modules/actor-core/src/core/kernel/dispatch/dispatcher/message_dispatcher.rs:122,132,177,182`
- **C（ライフタイム延長 / 明示 drop）**:
  - `modules/actor-core/src/core/kernel/dispatch/mailbox/mailbox_instrumentation/tests.rs:21,30,31`
  - `modules/actor-core/src/core/kernel/dispatch/mailbox/base/tests.rs:273`
  - `modules/actor-adaptor-std/src/std/dispatch/dispatcher/pinned_executor.rs:65`
- **D（ルール §29 の許容例外該当・コメント要追加）**:
  - `modules/actor-core/src/core/kernel/dispatch/mailbox/bounded_priority_message_queue.rs:62`（Pekko 互換の先行コメントあり）
  - `modules/actor-core/src/core/kernel/dispatch/mailbox/bounded_stable_priority_message_queue.rs:67`
  - `modules/actor-core/src/core/kernel/dispatch/mailbox/system_queue.rs:146`
  - `modules/actor-core/src/core/kernel/dispatch/mailbox/base.rs:281`
- **E（対象外）**: `///` doc コメント内のコード例（例: `modules/actor-core/src/core/kernel/actor/actor_cell.rs:59`）

## 4. 要件分解

| # | 要件 | 種別 | 由来 |
|---|------|------|------|
| 1 | `let _ = <Result/Option/#[must_use]>;` による握りつぶしを全件解消する | 明示 | ルール MUST NOT |
| 2 | `.ok();` 式文による Result のエラー握りつぶしを全件解消する | 明示 | ルール MUST NOT |
| 3 | 握りつぶしに見える `let _ = ...;` のうち引数破棄・ライフタイム延長・明示 drop を意図が明示される記法に置き換える | 暗黙（要件 1 由来） | 機械的強制のためには `let _ =` パターン自体を禁止する必要がある |
| 4 | ルール §29 の許容例外には直前コメントで安全性を明示する | 明示 | ルール「許容例外」節 |
| 5 | 既存の 2 lint で検出漏れするパターンを機械的に検出する強制手段を追加する | 暗黙（要件 1,2 由来） | 修正のみでは再発防止にならない |
| 6 | 機械的強制を `scripts/ci-check.sh` 経由の CI に組み込む | 暗黙（要件 5 由来） | 既存 Dylint 群と同じ配線に載せる |

## 5. スコープ

### 5.1 対象

- **ファイル範囲**: `modules/**/*.rs`
  - production コード（`src/**`）
  - 統合テスト（`tests/**`）
  - モジュール内 `tests.rs`
- **対象モジュール**: `actor-core`, `actor-adaptor-std`, `cluster-core`, `cluster-adaptor-std`, `persistence-core`, `remote-core`, `remote-adaptor-std`, `stream-core`, `stream-adaptor-std`, `utils-core`, `utils-adaptor-std`
- **違反箇所**: A / B / C / D 全カテゴリ

### 5.2 対象外

| 項目 | 除外理由 |
|------|---------|
| `references/` 配下（protoactor-go, pekko） | 外部コードのため修正対象外 |
| `showcases/std` | 本 lint の適用是非は別判断（本計画では触れない） |
| `///` doc コメント内のコード例 | 実行されないため品質保証対象外。新 lint でも HIR 上対象外 |
| `lints/*-lint/` 自身の実装コード | `dylint-link` 経由ビルドで workspace.lints 適用外。別フェーズ判断 |
| `.agents/rules/ignored-return-values.md` の改訂 | 現ルール準拠が目的でありルール自体の変更は要求外 |
| 公開 API の契約変更 | 内部実装の失敗観測可能性のみ強化（外部影響なし） |

## 6. 設計方針

### 6.1 新 Dylint `let-underscore-forbid-lint` の新設

既存 2 lint（`unused_must_use`, `clippy::let_underscore_must_use`）では P1〜P5 を取りこぼすため、より厳格な独自 lint を新設する。

#### 検出ルール

| 検出 | 対象 |
|------|------|
| D1 | `let _ = <expr>;` 形式の `Local` ノード全般（右辺の型に関係なく発火） |
| D2 | `<expr>.ok();` の式文（メソッド名が `ok` で引数ゼロ、レシーバ型が `Result`） |

D1 の「右辺の型に関係なく」は、P1〜P5 をすべて捕捉するため意図的に広く設計する。
意図ある例外（ルール §29 該当ケース）は、直前コメント `// must-ignore: <理由>` で許容する。

#### 例外コメント規約

- 形式: `// must-ignore: <理由>`（プレフィックスを厳密に一致させる）
- 配置: 違反行の**直前行**（間に空行不可）
- 理由: ルール §29 のどの例外カテゴリに該当するかを一文で記述
- 機械的検証: 新 lint が `SourceMap` から直前行を読み取り、プレフィックス一致で例外判定

#### 除外対象

- `///` / `//!` doc コメント内: HIR 上ノードにならないため自動除外
- `target/` 配下: `should_ignore` で除外（`ambiguous-suffix-lint` と同様）
- 新 lint 自身の `tests/ui/` 配下のフィクスチャ: 除外パスに追加

#### `#[cfg(test)]` の扱い

除外しない。テストコード内でも握りつぶしを禁止する（カテゴリ A のサンプルにテスト違反が多数含まれるため）。

### 6.2 既存違反の書き換え方針（カテゴリ別）

| カテゴリ | 方針 | 具体手段 |
|---------|------|---------|
| A | 失敗を観測可能にする | `?` 伝播、`if let Err(e) = ... { warn!("...") }`、テストは `.expect("...")`、shutdown best-effort は `// must-ignore:` コメント付き維持 |
| B | 引数破棄自体を不要にする | trait impl の仮引数を `_name` 形式にリネームし `let _ = (...);` 自体を削除 |
| C | 意図を明示する | `drop(x);` に置換、RAII ガードは `let _guard = ...;` に命名 |
| D | 例外コメントを付与 | `// must-ignore: <理由>` を直前行に追加、または `drop(...)` 明示 |

### 6.3 `no_std` / `std` 経路の遵守

カテゴリ A でログ観測可能化する際の経路選択。

- `modules/*-core/`（`no_std`）: `tracing` への新規直接依存は禁止。既存 `EventStream` / `log_event` 経路を使用
  - 参考: `modules/actor-core/src/core/kernel/event/stream/event_stream_events.rs`, `modules/actor-core/src/core/kernel/event/logging/`
- `modules/*-adaptor-std/`（`std`）: `tracing::warn!` 使用可

### 6.4 検討したアプローチと採否

| アプローチ | 採否 | 理由 |
|-----------|------|------|
| 既存違反を全件修正のみ（lint 追加なし） | 不採用 | 再発防止なき修正は品質保証にならない（要件 5,6 不充足） |
| `clippy::let_underscore_untyped` / `let_underscore_future` / `let_underscore_lock` を追加 | 不採用（fallback として保持） | P2（`.ok();`）と P3（引数破棄）は取りこぼす。Q1 で新 Dylint 不可の判断が出た場合の代替案 |
| 新 Dylint `let-underscore-forbid-lint` を追加、`// must-ignore:` で例外許容 | **採用** | P1〜P5 全パターン捕捉。ルール §29 の「例外はコメント必須」を機械検証可能 |
| `let _ = x;` → `drop(x);` の一括機械置換 | 不採用 | Result を drop しても Err は消える。盲目的置換は禁止 |
| スコープを特定モジュール（例: `actor-core`）に先行縮小し段階実施 | 採否は Q2 に依存 | 560 箇所一括 PR は review 困難だが、Dylint 導入との同期を取る必要がある |

## 7. 実装方針

### 7.1 ステップ

1. **新 Dylint 作成**
   - `lints/let-underscore-forbid-lint/` を新設
   - `lints/ambiguous-suffix-lint/` をテンプレートに `Cargo.toml` / `rust-toolchain.toml` / `src/lib.rs` / `tests/ui.rs` / `tests/ui/` を作成
   - 検出ルール D1, D2 を実装
   - 直前行コメント `// must-ignore: <理由>` の正規表現検査を実装
2. **CI 配線**
   - ルート `Cargo.toml` `[workspace.metadata.dylint].libraries` に `{ path = "lints/let-underscore-forbid-lint" }` を追加
   - `scripts/ci-check.sh` L654-665 の `lint_entries` 配列に `"let-underscore-forbid-lint:lints/let-underscore-forbid-lint"` を追加
3. **既存違反の修正（カテゴリ別）**
   - A: 失敗観測可能化
   - B: 仮引数リネーム
   - C: `drop(...)` / `let _guard` への置換
   - D: `// must-ignore:` コメント付与
4. **検証**
   - `./scripts/ci-check.sh ai dylint let-underscore-forbid-lint` で全違反が解消されることを確認
   - `final-ci` ムーブメントで `./scripts/ci-check.sh ai all` を実行

### 7.2 参照すべき既存実装

| 参照先 | 用途 |
|-------|------|
| `lints/ambiguous-suffix-lint/` | Cargo.toml 形式、lib.rs 全般構造、`should_ignore` の `target` 除外 |
| `lints/use-placement-lint/` | `Local` / 式文ノードの AST 走査 |
| `lints/rustdoc-lint/` | HIR 経由の doc コメント除外（本件では AST レベルで自動除外なので参考程度） |
| `modules/actor-core/src/core/kernel/event/stream/event_stream_events.rs` | `no_std` 互換ログ経路 |
| `modules/actor-core/src/core/kernel/dispatch/mailbox/bounded_priority_message_queue.rs:61` | 既存の自由記述コメント例（`// must-ignore:` への統一対象） |

### 7.3 影響範囲（配線が必要な箇所）

| # | 箇所 | 変更内容 |
|---|------|---------|
| 1 | `lints/let-underscore-forbid-lint/` | 新規作成（`Cargo.toml`, `rust-toolchain.toml`, `src/lib.rs`, `tests/ui.rs`, `tests/ui/`） |
| 2 | ルート `Cargo.toml` `[workspace.metadata.dylint].libraries` | 1 行追加 |
| 3 | `scripts/ci-check.sh` L654-665 `lint_entries` | 1 行追加 |
| 4 | カテゴリ A の Result 伝播を行う関数シグネチャ | 呼び出し元チェーンに `Result` が伝播した関数のみ、伝播先すべてを修正 |
| 5 | カテゴリ B の trait default impl の仮引数 | impl 側のみリネーム（trait 定義と呼び出し元は無変更） |
| 6 | カテゴリ A のログ追加箇所（`no_std`） | `EventStream::log_event` 等の既存経路を呼び出し。新規 `tracing` 依存追加は禁止 |

### 7.4 到達経路・起動条件

| 項目 | 内容 |
|------|------|
| 利用者向け入口 | なし（公開 API 変更なし。内部品質強化のみ） |
| 起動条件 | CI 実行時 `./scripts/ci-check.sh ai dylint` で新 lint が自動起動 |
| 未対応項目 | なし |

## 8. 実装時の注意（アンチパターン）

- `#[allow(clippy::let_underscore_must_use)]` / `#[allow(unused_must_use)]` / 新 lint への `#[allow(...)]` で違反を通すことは禁止（CLAUDE.md「lint エラーを安易に allow で回避しない」）
- `let _ = x;` → `drop(x);` の盲目的一括置換を行わない（右辺が Result なら drop しても Err は消える）
- `.ok();` をプロダクションコードで `.expect(...)` に置換する場合、panic 条件が本当に到達不能か検証すること
- `let _ = (a, b);` はタプル展開せず、**仮引数名を `_a`, `_b` に直接リネームする**（冗長な bind を残さない）
- 新 Dylint の検出ルールに既存 `lints/*/ui/` テストフィクスチャが衝突しないか事前点検
- `no_std` モジュールで fire-and-forget をログ化する際、`tracing` への新規依存は追加しない（既存 `EventStream` 経路を使う）
- 新 lint 名 `let-underscore-forbid-lint` は曖昧サフィックス禁止ルールに該当しない（Util/Manager 系でない動詞サフィックス `-lint` は既存の 10 lint すべてが採用する規約）

## 9. 確認事項（ユーザー判断を要する項目）

本計画の実装着手前に、以下 3 点についてユーザー判断が必要。
本計画では推奨判断を併記し、ユーザー指示がない場合のデフォルト動作を明示する。

### Q1: 新 Dylint 追加の可否

- **問**: CLAUDE.md 「`.claude/rules/rust/` に集約されている。変更する場合は人間から許可を取ること」および「lint エラーを安易に allow で回避しない。allow を付ける場合は人間から許可を得ること」に準じ、新 lint の追加は人間許可が必要と解釈される。新 `let-underscore-forbid-lint` の追加を許可してよいか。
- **推奨**: **追加を許可**。理由は §3.2 に示したとおり、既存 2 lint では P1〜P5 を取りこぼし、再発防止（要件 5,6）が達成できないため。
- **不許可の場合の代替**: `[workspace.lints.clippy]` に `let_underscore_untyped = "deny"` / `let_underscore_future = "deny"` / `let_underscore_lock = "deny"` を追加。ただし P2（`.ok();`）と P3（引数破棄）は機械検出不可となり、コードレビューでの運用カバーが必要。

### Q2: 560 箇所一括修正 vs 段階実施

- **問**: 新 Dylint 追加と同一 PR/ワークフロー内で全 560 箇所を修正するか、あるいは pilot モジュール（例: `actor-core` 単独）で先行実施し後続モジュールは別ワークフローに分割するか。
- **推奨**: **pilot 先行 → 全モジュール展開**の 2 段階。第 1 段では `actor-core` のみを修正し、新 Dylint 追加 + `actor-core` の違反解消 + CI green を達成。第 2 段以降で残りのモジュールを順次対応。理由は以下。
  - 560 箇所一括 PR はレビュー困難
  - Dylint を先に `deny` 設定すると全モジュールで CI 赤化するため、段階展開時は一時的にモジュール単位で `#[allow]` を付けて段階剥がしする運用が必要となり、ルール違反（allow を付ける）になる
  - pilot で「Dylint + 全修正」を 1 セットで閉じ、module ごとに CI を走らせて段階 merge する運用を推奨
- **一括実施を選ぶ場合の留意点**: 560 箇所をカテゴリ別に集計し、機械置換可能な B, C, D から先に処理。A は人間判断を要するため最後にレビュー集中。

### Q3: `// must-ignore:` コメント規約の新設可否

- **問**: プロジェクト既存の許容例外コメントは自由記述（例: `modules/actor-core/src/core/kernel/dispatch/mailbox/bounded_priority_message_queue.rs:61` の「Pekko 互換: ...」）。これを `// must-ignore: <理由>` プレフィックス規約に統一してよいか。
- **推奨**: **プレフィックス規約を新設**。理由は自由記述では機械的検証が困難なため。既存の自由記述コメント（§3.3 カテゴリ D）は本計画の対象範囲内で `// must-ignore: <理由>` 形式に書き換える。
- **既存コメント温存を選ぶ場合の代替**: 新 Dylint の例外検出を「直前行に任意のコメントがあれば例外とする」まで緩めるか、あるいは例外許容自体を廃止し全件コード修正にする。後者の場合、`Vec::pop` 相当パターン（ルール §29 許容例外 ③）も書き換えが必要となり、標準ライブラリ API の自然な使用が阻害される。

## 10. スコープ外（明示的に対応しない項目）

- 公開 API の契約変更
- `references/` 配下
- `showcases/std`
- `///` doc コメント内のコード例
- `lints/*-lint/` 自身の実装コード
- `.agents/rules/ignored-return-values.md` の改訂

## 11. 実装進捗

### 11.1 Phase 0: lint 本体作成 (完了)

- `lints/let-underscore-forbid-lint/`
  - `Cargo.toml`, `rust-toolchain.toml`, `src/lib.rs`, `tests/ui.rs`, `tests/ui/`
  - 検出ルール D1 (`let _ = ...`) と D2 (`Result::ok();` の式文) を実装
  - 例外コメント: `// must-ignore: <理由>` を違反行の直前行に配置 (空行不可)
  - `lints/ambiguous-suffix-lint/` をテンプレートとして参照
  - `cargo build --release` OK、手動実行で workspace 全体に対する違反検出を確認

### 11.2 Phase 0.5: pilot 範囲 (この PR)

**目的**: lint 本体の準備と、比較的影響が浅い `utils-core` / `actor-core` の違反を全件解消する。後続 PR で CI 配線 + 他 crate 修正を行う。

件数サマリ (合計 **21 件**):

| カテゴリ | 合計 | utils-core | actor-core |
|----------|-----:|-----------:|-----------:|
| **A** (`// must-ignore:`) | 6 | 0 | 6 |
| **B** (仮引数リネーム `_foo`) | 7 | 1 | 6 |
| **C** (`drop(...)` 明示) | 4 | 0 | 4 |
| **D** (`drop(xs.pop())` で `Vec::pop` / `BinaryHeap::pop` の `Option<T>` を破棄) | 4 | 2 | 2 |

修正済み crate:
- `utils-core` (3 件): `arc_shared.rs:106` (B), `binary_heap_priority_backend.rs:81` (D), `vec_deque_backend.rs:83` (D)
- `actor-core` (18 件):
  - B: 仮引数リネーム — `actor_ref_provider/base.rs:114`, `message_dispatcher.rs` x4, `mailbox/base.rs:256` (引数 `_throughput_deadline`)
  - C: `drop(...)` 明示 — `context_pipe/waker.rs:54`, `failure_message_snapshot.rs:60/61`, `system_queue.rs:146`
  - D: `drop(xs.pop())` — `bounded_priority_message_queue.rs:62`, `bounded_stable_priority_message_queue.rs:67`
  - A: `// must-ignore:` コメント付与 — `scheduler/delay_provider.rs:48`, `scheduler_core.rs:242/351/445`, `mailbox_queue_state.rs:64/68`

### 11.3 Phase 1: 後続 PR のスコープ

- `Cargo.toml` の `[workspace.metadata.dylint].libraries` に `{ path = "lints/let-underscore-forbid-lint" }` を追加
- `scripts/ci-check.sh` の `lint_entries` に `"let-underscore-forbid-lint:lints/let-underscore-forbid-lint"` を追加
- 残 crate の違反修正 (lint 実行時にさらに奥の crate で検出されるものを順次):
  - `actor-adaptor-std` (1 件 + 追加検出)
  - `remote-core` (6 件 + 追加検出)
  - `cluster-core` (13 件 + 追加検出)
  - `cluster-adaptor-std`
  - `persistence-core`
  - `stream-core` (32 件 + 追加検出、最大級)
  - `stream-adaptor-std`
- 残テストコード (約 500 件) の修正

### 11.4 確定仕様: `// must-ignore:` は 1 行固定

- `// must-ignore:` コメントは **1 行で記述することが仕様として確定** している。lint は違反行の直前 1 行のみを参照する
- 根拠: ルール §29 の許容例外は 1 文で書ける粒度を前提としており、複数行必須な説明が必要なら設計自体を見直すべきサイン
- rustfmt の `wrap_comments = true` + `comment_width = 100` で自動折り返されないよう、インデント込みで 100 文字以内に収める
- 100 文字を超えて説明したいケースは、コードを分割するか、別途 `///` ドキュメントコメントで補足する
- 本プロジェクト外部環境で `lints/*/tests/ui/` の `cargo test --test ui` が libgit2 / cargo-platform 依存で失敗する。ci-check.sh の L770-774 経由では動作するが、手動 `cargo test` では再現性がない。UI テストの充実は現環境制約の解決と同時に対応
