## Context

`modules/actor-core/Cargo.toml:24` に以下が宣言されている:

```toml
portable-atomic = { workspace = true, default-features = false, features = ["critical-section"] }
```

`portable-atomic/critical-section` feature は、`AtomicU64` などの幅広 atomic を **ハードウェアサポートなしのターゲット** で fallback 提供するため `critical-section` ベースの emulated atomic を使う。これにより組み込み 32-bit (例: `thumbv6m-none-eabi`、`riscv32i-unknown-none-elf`) でも `AtomicU64` が使えるようになる。

ただし以下が未確認:

- fraktor-rs は実際に組み込み 32-bit ターゲットを CI / 配布対象にしているか
- 利用者が組み込み環境で運用している実例があるか
- `actor-core/src/` 内の `portable_atomic::Atomic*` 利用箇所はどの幅 (`U64` / `Usize` / `U32`) を要求しているか
- 仮にすべて 32-bit 幅で済むなら、組み込み 32-bit ターゲットでも `core::sync::atomic` で代替可能 (= `portable-atomic` 自体不要 or `critical-section` feature 不要)

`portable-atomic/critical-section` feature が **真に必要か否か** を本 change で評価し、step08 で実施する変更 (退役 / 維持 / 条件付き維持) を確定させる。本 change は **コードを一切変更しない調査 change**。

## Goals / Non-Goals

**Goals:**

- fraktor-rs が CI / 配布対象としているターゲット集合を棚卸し
- workspace 全体で `portable_atomic::Atomic*` が使われている箇所と要求幅を列挙
- `core::sync::atomic` 代替可否を一次評価
- 退役 / 維持 / 条件付き維持の意思決定マトリクスを作成
- step08 で採るべきアクションを Recommendation として明示
- 評価成果物を `docs/plan/2026-04-22-portable-atomic-critical-section-evaluation.md` として残す

**Non-Goals:**

- コード変更 (`Cargo.toml` 修正、`core::sync::atomic` 置換は step08 のスコープ)
- `portable-atomic` 本体依存の削除 (`portable-atomic` は `heapless` 等から推移的にも引き込まれるため、feature 指定だけが対象)
- 組み込みクロスコンパイル実機検証 (CI 拡張やローカル `cargo build --target ...` 検証は step08)
- step01〜step06 で扱った test-support / spin / critical-section 関連の再評価 (完了済み)

## Investigation Methodology

### Phase 1: ターゲット棚卸し (15 分目安)

`.github/workflows/**.yml` を全数走査し、以下を抽出:

- `cargo check / build / test` の `--target` オプションに登場するターゲット triple
- `dtolnay/rust-toolchain` の `targets:` 入力
- `cross` / `cargo-zigbuild` 等の言及
- 組み込み系の言及 (`thumbv*`、`riscv32*`、`avr`、`xtensa`、`ARMv6-M` 等)

成果: ターゲット triple のリストを `evaluation.md` に記録。AtomicU64 ハードサポート有無で分類:

- 64-bit ターゲット (x86_64-*、aarch64-*) → ハードサポートあり
- 32-bit ターゲットで AtomicU64 サポート (i686-pc-windows-* 等) → ハードサポートあり
- 32-bit ターゲットで AtomicU64 emulation 必要 (thumbv6m、thumbv7m、riscv32i 等) → `portable-atomic/critical-section` 必要

### Phase 2: 利用箇所の census (30 分目安)

```bash
rg -n 'portable_atomic::' modules/ --glob '*.rs'
rg -n 'use portable_atomic' modules/ --glob '*.rs'
```

各ヒット箇所について以下を表で記録:

| ファイル | symbol | 要求幅 | 代替候補 (`core::sync::atomic`) | 備考 |

要求幅:

- `AtomicU64` → 64-bit、emulation 対象
- `AtomicUsize` → ターゲット幅依存 (32-bit ターゲットでは `core::sync::atomic::AtomicUsize` で十分)
- `AtomicU32` / `AtomicI32` → 32-bit、すべてのターゲットで `core::sync::atomic` あり
- `AtomicBool` / `AtomicPtr` → 全ターゲット OK

代替候補列で「`core::sync::atomic::AtomicXX` で代替可能」「不可 (理由)」を判定。

### Phase 3: 影響度評価 (10 分目安)

Phase 1 と Phase 2 の結果から、`portable-atomic/critical-section` feature を退役した場合の影響を評価:

- ケース A: すべての利用箇所が 32-bit 幅で済む → 退役可
- ケース B: 1 箇所でも `AtomicU64` 利用があり、かつ 32-bit 組み込みターゲットが対象に含まれる → 維持必須
- ケース C: `AtomicU64` 利用はあるが 32-bit 組み込みターゲットが対象外 → 条件付き維持 / 退役可 (将来需要の判断)

### Phase 4: 利用実績調査 (任意、15 分目安)

GitHub の issues / discussions / README / 配布先 (crates.io download targets) から、組み込みユーザの存在を確認:

- `gh issue list --search "embedded"` 等
- README / docs の embedded 言及
- crates.io の download stats (ターゲット別は出ないが、量で組み込み比率を推測)

「実利用報告ゼロ」なら退役寄り、「実利用報告あり」なら維持寄り。

## Decision Matrix

評価レポート末尾に以下のマトリクスを記載する:

| 選択肢 | 内容 | Pros | Cons | step08 で採るべき場合 |
|--------|------|------|------|----------------------|
| **X: 退役** | `portable-atomic/critical-section` feature を削除し、必要なら利用箇所を `core::sync::atomic` に置換 | 依存縮小、`critical-section` 推移依存解消、組み込み非対象が明文化される | 将来 32-bit 組み込み対応時に再導入コスト発生 | ケース A (利用箇所がすべて 32-bit 幅で済む) または ケース C で組み込み非対象を確定 |
| **Y: 維持** | 現状維持。`portable-atomic/critical-section` feature を保持 | 32-bit 組み込み運用が壊れない、将来的な拡張余地を保持 | `critical-section` 推移依存が残る、emulation コストが乗る | ケース B (32-bit 組み込みが対象、かつ `AtomicU64` 利用あり) |
| **Z: 条件付き維持** | actor-core feature flag (例: `embedded-atomic-fallback`) で `portable-atomic/critical-section` を opt-in 化 | std/64-bit 系のデフォルト使用感を改善、組み込み利用者は opt-in | feature 設計コスト、誤用 (opt-in 忘れ) リスク | ケース C で「組み込み対応の意思はあるが現状非対象」 |

各列の判定根拠を Phase 1〜4 の結果から導出。

## Spec Delta Rationale

本 change は **純粋な調査 change** (Non-Goals でコード変更を明示禁止) のため、spec delta は形式的な最小限にとどめる (proposal 案 C)。実質的な spec 変更は step08 で行う。

最小限 delta として `actor-lock-construction-governance` capability の既存 Requirement「actor-* の Cargo.toml は primitive lock crate を non-optional な直接依存として宣言してはならない」に **Scenario を 1 件追加** する:

- low-level utility crate (`portable-atomic` 等) の feature 指定は用途ターゲットが明示されなければならない
- 評価レポート (`docs/plan/`) または Cargo.toml コメントで justification されている

これにより「将来また同種の `features = [...]` を追加するとき、根拠を残す」というガードが spec で機械的に検証可能になる。

## Output Artifacts

- `docs/plan/2026-04-22-portable-atomic-critical-section-evaluation.md`:
  - Investigation summary (Phase 1〜4 の結果)
  - Decision matrix (上記表)
  - Recommendation (X / Y / Z のいずれか + 根拠)
  - step08 へのハンドオフ (具体的なアクションリスト)
- 上記レポートへの参照を `docs/plan/2026-04-21-actor-core-critical-section-followups.md` 残課題 2 から追加

## Risks / Trade-offs

- **[Risk] 評価が表面的に終わり step08 判断を誤る**:
  - Mitigation: Phase 2 (census) で全ヒット箇所を網羅的に列挙、要求幅を機械的に判定。曖昧な箇所は `[要確認]` マークを付けて step08 で再確認
- **[Trade-off] Phase 4 (利用実績調査) はコストが見合わない可能性**:
  - 受容: 「実績ゼロ → 退役」は弱い根拠。Phase 1〜3 で結論が出るならスキップ可。レポートに「Phase 4 はスキップ」と明記
- **[Risk] `portable_atomic::AtomicU64` 利用が 1 箇所でもあると組み込み非対応を確定する判断が必要**:
  - Mitigation: ケース C のための条件付き維持 (案 Z) を Decision Matrix に含める。step08 で詳細決定

## Migration Plan

調査 change のため migration なし。以下の順序で実施:

1. **Phase 1**: CI ターゲット棚卸し → `.github/workflows/` を grep
2. **Phase 2**: portable_atomic 利用 census → `rg portable_atomic`
3. **Phase 3**: 代替可否評価 → 表を埋める
4. **Phase 4** (任意): 利用実績調査
5. **レポート執筆**: `docs/plan/2026-04-22-portable-atomic-critical-section-evaluation.md`
6. **Decision Matrix**: 上記表を埋める
7. **Recommendation**: X / Y / Z を確定
8. **Spec delta apply** (案 C minimal Scenario 追加)
9. **Validate**: `openspec validate step07-... --strict`
10. **docs/plan の hand-off メモ更新**: 残課題 2 に評価レポートへのリンクを追加
11. **Commit / PR / merge / archive**

## Open Questions

- 評価結果が「ケース C: 条件付き維持」になった場合、step08 では feature flag 設計の追加コストが発生する。本 change の Recommendation では「X / Y / Z のいずれか + 設計概略」までに留め、詳細設計は step08 design.md で行う
- step08 が「中止 (= 維持)」になった場合、本 change の Recommendation は「Y: 維持。理由は ... 」で完結し、step08 は archive 前に proposal で「中止判断」を明記して archive
