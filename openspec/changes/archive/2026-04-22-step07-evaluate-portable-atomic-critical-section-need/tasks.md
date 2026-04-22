## 1. Phase 1 — CI ターゲット棚卸し

- [x] 1.1 `.github/workflows/` 配下の `**.yml` を全数走査し、`--target` / `targets:` / `cross` / `cargo-zigbuild` の言及を抽出
- [x] 1.2 抽出したターゲット triple を一覧化し、AtomicU64 ハードサポート有無で 3 分類 (64-bit / 32-bit hard / 32-bit emulation 必要)
- [x] 1.3 評価レポートに「Phase 1: CI ターゲット棚卸し」セクションとして記録

## 2. Phase 2 — portable_atomic 利用 census

- [x] 2.1 `rg -n 'portable_atomic::' modules/ --glob '*.rs'` で全ヒット箇所を列挙
- [x] 2.2 `rg -n 'use portable_atomic' modules/ --glob '*.rs'` で import 形式の参照も列挙
- [x] 2.3 各ヒットを表化: `| ファイル | symbol | 要求幅 | 代替候補 (core::sync::atomic) | 備考 |`
- [x] 2.4 要求幅判定: `AtomicU64` / `AtomicUsize` / `AtomicU32` / `AtomicI32` / `AtomicBool` / `AtomicPtr` のどれか
- [x] 2.5 代替候補列に「`core::sync::atomic::AtomicXX` で代替可能」「不可 (理由)」を記入
- [x] 2.6 評価レポートに「Phase 2: 利用 census」セクションとして記録

## 3. Phase 3 — 影響度評価

- [x] 3.1 Phase 1 + Phase 2 の結果から、`portable-atomic/critical-section` 退役時の影響をケース分類:
  - ケース A: すべての利用箇所が 32-bit 幅で済む → 退役可
  - ケース B: 1 箇所でも `AtomicU64` 利用があり、かつ 32-bit 組み込みターゲットが対象 → 維持必須
  - ケース C: `AtomicU64` 利用はあるが 32-bit 組み込みターゲットが対象外 → 条件付き維持 / 退役可
- [x] 3.2 確定ケースを評価レポートに記録

## 4. Phase 4 — 利用実績調査 (任意、結論に必要なら実施)

- [x] 4.1 GitHub issues / discussions で "embedded" / "no_std" 関連の言及を軽く確認 (`gh issue list --search ...`) — **スキップ** (Phase 3 でケース B 確定、結論に必要なし)
- [x] 4.2 README / docs で組み込み環境の言及を確認 — **スキップ** (同上)
- [x] 4.3 評価レポートに「Phase 4: 利用実績調査」セクションを追加 (スキップした場合は明記)

## 5. Decision Matrix と Recommendation 作成

- [x] 5.1 評価レポートに Decision Matrix (X: 退役 / Y: 維持 / Z: 条件付き維持) を表として記載
- [x] 5.2 Phase 1〜3 (4) の結果から **Recommendation** (X / Y / Z のいずれか + 根拠) を執筆
- [x] 5.3 step08 で採るべきアクション (具体的な commit / Cargo.toml 修正候補など) を Recommendation セクションに列挙

## 6. 評価レポート完成

- [x] 6.1 `docs/plan/2026-04-22-portable-atomic-critical-section-evaluation.md` を完成 (Phase 1〜5 + Recommendation を含む)
- [x] 6.2 `docs/plan/2026-04-21-actor-core-critical-section-followups.md` 残課題 2 から本評価レポートへのリンクを追加
- [x] 6.3 step08 proposal を確認し、本評価レポートの Recommendation を反映 (中止判断含む)

## 7. Spec delta apply

- [x] 7.1 `openspec/specs/actor-lock-construction-governance/spec.md` の MODIFIED Requirement を確認し、新規 Scenario「low-level utility crate の feature 指定は対象ターゲット / ユースケースが明示されている」を main spec に sync (本 change archive 時に実施)
- [x] 7.2 新規 Scenario が要求する justification (Cargo.toml コメント or 評価レポート参照) を `modules/actor-core/Cargo.toml` の `portable-atomic` 行に追加 (※ コード変更は本 change の Non-Goals に該当するが、Scenario 違反になるため最小限のコメント追加は本 change で行う)

## 8. 全体検証

- [x] 8.1 `openspec validate step07-evaluate-portable-atomic-critical-section-need --strict` で artifact 整合確認
- [x] 8.2 `cargo test --workspace` (コード変更は Cargo.toml コメントのみのため pass する想定)
- [x] 8.3 `./scripts/ci-check.sh ai all` で全 CI 緑

## 9. コミット・PR

- [x] 9.1 ブランチ作成: `step07-evaluate-portable-atomic-critical-section-need`
- [x] 9.2 論理単位での commit (artifacts / 評価レポート / Cargo.toml コメント / docs/plan 更新)
- [x] 9.3 push + PR 作成 (base: main、title prefix `docs(actor-core):` または `chore(actor-core):`)
- [x] 9.4 CI 全 pass + レビュー対応 + マージ
- [x] 9.5 archive (`/opsx:archive` または skill 経由)
