## Context

`actor-core` の `tick_driver` モジュール内 `tick_feed.rs` は、tick イベントの内部キュー保護のために `critical_section::Mutex<RefCell<VecDeque<u32>>>` を直接 use している。これは以下の構造的問題を生んでいる:

1. **`utils-core` 抽象の素通り**: `actor-core` 内・`cluster-core` 内で 27 箇所超が `SharedLock::new_with_driver::<DefaultMutex<_>>(...)` パターンを使っているのに対し、tick_feed のみが直接 `critical-section` クレートに依存している
2. **`actor-core/Cargo.toml:24` の non-optional な直接依存**: 上記の唯一の利用者のために `critical-section = { workspace = true, default-features = false }` がトップレベル依存（non-optional）として残っている
3. **`test-support` feature の存在理由の片方**: `actor-core/Cargo.toml:19` の `test-support = ["critical-section/std"]` は、std 環境でのテスト実行時に `critical-section` の impl provider を提供する必要があるために存在する。tick_feed が直接 `critical_section::Mutex` を要求していることがその一因
4. **既存ガバナンスとの逸脱**: `actor-lock-construction-governance` spec が「`actor-*` の production code は固定 backend 指定や直接 lock 構築を行ってはならない」と定めているが、`critical_section::Mutex` の直接 use は文言上カバーされておらず、ガバナンスの精神には反する

「源を絶つ」= `actor-core` の **production code（ソースコード）から `critical-section` クレートへの直接利用を撤去** することを目的とする。Cargo features 構文制約により `[dependencies]` の `critical-section` エントリ自体は完全削除できず `optional = true` で残す（詳細は Decision 6）。`portable-atomic` 経由の間接依存は本 change の対象外（`portable-atomic` のような low-level crate は別途 lint allow-list で扱う）。

## Goals / Non-Goals

**Goals:**
- `actor-core` の production code から `critical-section` クレートへの直接 use を撤去し、`utils-core` 抽象（`SharedLock + DefaultMutex`）経由に置換する
- `actor-core` から `critical-section` への direct dependency edge を **通常ビルドで無効化** する（Cargo features 構文制約により宣言自体は `optional = true` で残す。詳細は Decision 6）
- `actor-lock-construction-governance` spec を拡張し、同種の逸脱を予防する 2 要件（primitive lock crate 直接 use 禁止 + Cargo.toml non-optional 直接依存禁止）を spec として宣言する
- `tick_feed.rs` の public API シグネチャ互換性を維持する（メソッドシグネチャ不変）
- 既存の単体・統合テストが従来どおり通る

**Non-Goals:**
- `test-support` feature 自体の撤去（impl provider 提供機能は維持する。実装表現を `dep:critical-section` 追加に調整するのみ）
- `actor-core/Cargo.toml` から `critical-section` エントリの完全削除（Cargo features 構文制約により optional として残す）
- `portable-atomic` の `critical-section` feature 撤去（組み込み 32-bit ターゲット向けの atomic fallback として必要）
- `actor-core` の他 16 ファイルでの `portable_atomic` 利用見直し
- `test-support` feature の責務 B（ダウンストリーム統合テスト用 API 公開）/ 責務 C（内部 API の pub 格上げ）の分離議論
- `utils-core` 側の同期抽象（`SpinSyncMutex`、`StdSyncMutex` 等）の変更
- ロックファミリ自動切り替えの新規メカニズム導入

## Decisions

### Decision 1: 抽象として `SharedLock + DefaultMutex` を採用する

**選択**: `queue: SharedLock<VecDeque<u32>>` とし、コンストラクタで `SharedLock::new_with_driver::<DefaultMutex<_>>(queue)` を使う。

**根拠**:
- `actor-core` および `cluster-core` の production code 27 箇所超で同一パターンが既に確立されている（例: `circuit_breaker_shared.rs:34`、`event_stream_subscriber_shared.rs:18`、`pool_router.rs`、`group_router.rs`）
- `DefaultMutex` は `utils-core/src/core/sync.rs:88-94` で feature ベースに backend 切り替えされる（`debug-locks` → `CheckedSpinSyncMutex`、`std-locks` → `StdSyncMutex`、それ以外 → `SpinSyncMutex`）。caller は backend を選ばない
- `actor-lock-construction-governance` spec が要求する「provider または抽象 boundary を通す」設計に準拠

**代替案と却下理由**:
- 案 A: `SpinSyncMutex<VecDeque<u32>>` を直接使う → ガバナンス違反（固定 backend 指定）。CI lint で検出されるべきパターン
- 案 B: `LockDriverFactory` 経由で provider 注入 → 過剰設計。`tick_feed` は actor-core 内部の固定構築要素であり、provider を外部から差し込む要件はない。`SharedLock::new_with_driver::<DefaultMutex<_>>` で feature 切り替えに任せるだけで十分
- 案 C: 自前で trait `TickFeedQueue` を切る → YAGNI。既存の `SharedLock` 抽象で十分

### Decision 2: `RefCell` 二重ラップを除去する

**選択**: `Mutex<RefCell<VecDeque<u32>>>` → `SharedLock<VecDeque<u32>>`。`RefCell` を取り除く。

**根拠**:
- `critical_section::Mutex` は `Sync` だが内部の値への可変アクセスを許さないため、`RefCell` で内部可変性を確保する必要があった
- `SharedLock::with_lock(|q| ...)` は closure に `&mut T` を直接渡す（`shared_lock.rs:72-85`）ため、`RefCell` は不要
- コードが 1 段薄くなる

### Decision 3: ISR セーフティ要件を厳密に再評価しない

**選択**: `SharedLock + DefaultMutex` への置換を行う。ISR セマンティクスの厳密化は本 change のスコープ外とする。

**根拠（事実ベース）**:
- 本 change 起案時に `enqueue_from_isr` の caller を `Grep` で全数調査した結果、唯一の caller は `modules/actor-core/src/core/kernel/actor/scheduler/tick_driver/tests.rs:83-84` のテストコードであり、**production code からの呼び出しは存在しない**
- 現状の `enqueue_from_isr` は `try_push` を呼ぶだけで、`enqueue` と完全に同じパスを通っている。ISR 専用の処理（割り込み禁止区間の明示、bare-metal 用 spin 戦略など）は一切実装されていない
- つまり「`critical_section::Mutex` を選んだ」という事実が ISR 安全性を担保している唯一の根拠だが、実際にはその担保を必要とする production caller が存在せず、API 名と実装意図の乖離が起きている
- `DefaultMutex` の no_std 既定は `SpinSyncMutex`（spin-wait）。`std-locks` 有効時は `std::sync::Mutex`（OS スケジューラ依存）。どちらも厳密な ISR 安全性は持たないが、production caller が存在しない以上実害はない
- 本 change は API 名 `enqueue_from_isr` を維持する（呼び出し側互換性のため）。API 名と実装の整合性、または ISR 用 backend 設計は別 change で扱う

**リスク許容理由**:
- 上記の grep 調査により、ISR 内からの呼び出しは production には存在しないことが確認済み
- 将来 ISR 用 backend が必要になれば、`DefaultMutex` の feature 切り替えに新たな variant（例: `irq-locks`）を追加する道が残る
- 厳密な ISR 安全性が必要なユーザーは、`tick_feed` を直接使わず独自実装を持つ判断ができる

### Decision 4: `actor-lock-construction-governance` を拡張する（新規 spec を作らない）

**選択**: 既存 spec `actor-lock-construction-governance/spec.md` に「primitive lock crate の直接 use 禁止」と「Cargo.toml 直接依存禁止」の 2 要件を ADDED Requirements として追加する。

**根拠**:
- 既存 spec の精神（lock backend を caller が固定しない）と完全に同一方向
- 関連 spec `compile-time-lock-backend` も `DefaultMutex` 利用を要求しており、本追加要件はこれを補完する形になる（compile-time-lock-backend は backend 切り替え機構の宣言、actor-lock-construction-governance は caller 側の使用規律、本追加は primitive crate 利用境界の明示）
- 新規 spec を作ると同じ趣旨の要件が 3 箇所に散らばる
- `critical-section`、`spin`、`parking_lot` といった primitive lock crate の直接 use 禁止は、既存ガバナンスの自然な拡張

**代替案と却下理由**:
- 案 A: 新規 spec `actor-core-no-direct-primitive-lock-crate` を作る → 既存 spec と趣旨重複、capability 粒度の細分化
- 案 B: 既存 spec を MODIFIED で書き換える → 既存要件はそのまま残り、新要件は ADDED で追記する形が自然

### Decision 5: `test-support` feature の機能を維持する（実装表現は Decision 6 で調整）

**選択**: `actor-core/Cargo.toml:19` の `test-support` feature の **機能**（std 環境での `critical-section` impl provider 提供）は維持する。

**根拠**:
- std 環境でテスト・bench・showcase を動かす際、`critical-section` の impl provider 選択（`std` feature 有効化）が必要なことは tick_feed の変更前後で変わらない（`portable-atomic` 経由の依存があるため）
- `test-support` feature 自体の責務分離（impl provider 提供 vs テスト用 API 公開 vs 内部 API 公開）は別 change で議論する
- Cargo features 構文制約への対応は Decision 6 の主選択 X に集約（実装表現の具体的調整方法）

### Decision 6: `critical-section` を `[dependencies]` から完全削除せず `optional = true` で残す

**選択（以降「主選択 X」と呼ぶ）**: `actor-core/Cargo.toml:24` の `critical-section = { workspace = true, default-features = false }` を `critical-section = { workspace = true, default-features = false, optional = true }` に変更する（**完全削除はしない**）。`test-support` feature は `["dep:critical-section", "critical-section/std"]` に変更する。

**背景となる Cargo features 構文制約**:
- Cargo の `<dep>/<feature>` 構文（例: `critical-section/std`）は、`<dep>` が `[dependencies]` または `[dev-dependencies]` に **直接宣言されている** ことを要求する
- 推移的依存（本件では `portable-atomic` 経由）に対しては `<dep>/<feature>` 構文を使えない
- もし `[dependencies]` から `critical-section` を完全削除すると、`test-support = ["critical-section/std"]` は `error: invalid feature ... dependency does not exist` になる

**根拠**:
- 「源を絶つ」の本来の目的は「actor-core の **production code（ソースコード）** が `critical-section` を直接 use しないこと」であり、Cargo.toml の `[dependencies]` エントリ自体の完全削除ではない
- `optional = true` にすれば、feature 無効時（通常ビルド）には `actor-core` から `critical-section` への **direct edge** は消える。ただし `Cargo.lock` 全体には `portable-atomic` 経由の transitive edge で `critical-section` が依然として存在する（これは Non-Goals）
- `test-support` feature 有効時のみ `actor-core` からの direct edge が現れ、`critical-section` の `std` feature が有効化される
- `dep:critical-section` を features 配列に明示することで、Cargo に「この feature が `critical-section` という optional dep を有効化する」ことを伝える（Cargo 1.60+ で導入された明示構文）

**代替案と却下理由**:
- 案 A: `critical-section` を `[dev-dependencies]` に移動 → bench / integration test では使えるが、`actor-core` 自身を library として使うダウンストリームの test code から `test-support` feature 経由で利用できなくなる。現状の利用パターンを壊す
- 案 B: `portable-atomic/critical-section-impl` のような portable-atomic 側の feature 経由で impl 有効化 → 起案時の起点理解では「`portable-atomic/critical-section` は逆に『`portable-atomic` が `critical-section` を使う』feature であり、『`critical-section` の `std` impl を有効化する』用途ではない」と推定。実機での確認は tasks 1.7 に委譲する。仮に該当 feature が見つかった場合は、案 X（optional 化）よりこちらが優れる可能性があるため、tasks 1.7 の結果次第で本 Decision を再検討する
- 案 C: `actor-core/build.rs` で `critical-section/std` を強制有効化 → 過剰複雑化。Cargo features の表現範囲で完結する **主選択 X**（optional + dep:critical-section 明示）が自然

**spec への影響**:
- Requirement 2（`Cargo.toml` 直接依存禁止）は「non-optional な直接宣言禁止、optional + feature gated は例外」と明示する必要がある
- 例外条項を spec に明記することで、本 change の処置と要件 B の整合を保つ

## Risks / Trade-offs

- **[Risk] ISR 優先度逆転の理論的可能性** → Mitigation: 現状の `enqueue_from_isr` が既に特別な ISR 配慮を持たないため、実害は理論上のみ。design.md Decision 3 で許容理由を明記。将来 ISR 用 backend が必要になれば `DefaultMutex` の feature variant 追加で対応可能
- **[Risk] `actor-lock-construction-governance` 拡張により既存コードに新規違反が見つかる可能性** → Mitigation: 本 change の対象は `tick_feed.rs` の修正と spec 拡張のみ。他 file の違反検出は CI lint 整備（別 change）で対応。本 change は「拡張要件を spec として宣言する」までを完了とする
- **[Trade-off] `DefaultMutex` の backend 選択は tick_feed の caller が指定できない** → 受容: そもそも `DefaultMutex` は workspace 全体で feature 統一切替されることを前提とした抽象。具体的には、tick_feed の caller が「ISR 安全性のために `critical_section::Mutex` ベースの mutex が欲しい」と思っても選択できない。しかし Decision 3 のとおり production caller が存在しないため実害なし

## Migration Plan

本 change はライブラリ内部実装の置換であり、ダウンストリーム移行手順は不要。

1. **Phase 1**: `tick_feed.rs` の実装置換と単体テスト確認
2. **Phase 2**: `actor-core/Cargo.toml` の `critical-section` を `optional = true` に変更し、`test-support` feature を `["dep:critical-section", "critical-section/std"]` に更新
3. **Phase 3**: `cargo build --features test-support` および `cargo build --no-default-features` の両方で成功することを確認
4. **Phase 4**: `openspec validate --strict` で artifact 整合確認、`openspec apply` 時の自動 merge を経て `actor-lock-construction-governance` spec に 2 要件（optional 例外条項を含む）が反映される
5. **Phase 5**: `./scripts/ci-check.sh ai all` 実行確認

ロールバックはソースコード変更のみであれば git revert で完結する。spec 変更を含む場合は、`openspec` の archive 機構との整合に注意（apply 後 archive 前であればファイル revert で十分）。

## Open Questions

- なし（必要な設計判断は本 design で確定済み）
