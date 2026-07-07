# Cluster Change Preflight

cluster モジュール（`cluster-core-kernel` / `cluster-core-typed` / `cluster-adaptor-std`）に触れる change を着手する前に、以下を確認する。

## 1. ドメイン用語（CONTEXT.md）

- [ ] 変更で導入・変更する概念が [CONTEXT.md](../../CONTEXT.md) の canonical term と一致している
- [ ] `_Avoid_` 欄の言い換えにドリフトしていない
- [ ] 新しいプロジェクト固有のドメイン用語がある場合、`/domain-modeling` または `/grill-with-docs` で確定してから `CONTEXT.md` に登録する（直接追加しない）

## 2. ADR 制約

cluster change では少なくとも以下を確認する。

| ADR | 制約 |
|-----|------|
| [0001](../adr/0001-failure-detector-configuration-contract.md) | `FailureDetectorConfig` は観測契約。アルゴリズム選択 API ではない |
| [0005](../adr/0005-cluster-message-serialization-reuses-actor-core.md) | cluster wire frame は actor-core serialization を source of truth とする |
| [0006](../adr/0006-sbr-decision-semantics-live-in-core.md) | SBR decision semantics は core に置き、std は lease backend のみ |

既存 ADR と矛盾する design / spec がある場合、黙って上書きせず矛盾を明示する。

## 3. 戦略文書

- [ ] [cluster-grain-runtime-roadmap](../plan/2026-05-25_cluster-grain-runtime-roadmap.md) の Grain 主軸方針と整合している（hidden singleton / ShardCoordinator 中心化を避ける）
- [ ] [cluster-gap-analysis](../gap-analysis/cluster-gap-analysis.md) の対象行を特定し、完了後に台帳を更新する

## 4. OpenSpec

公開 contract / operational invariant を変える change では:

- [ ] `openspec/changes/<change-id>/` に proposal / design / tasks / spec delta を作成する
- [ ] config / wrapper / setup key だけの change は作らない（所有する runtime logic と同梱する）
- [ ] 完了時に `openspec validate --strict` を通す

## 5. Kiro spec（feature-level）

大きな feature 追加では:

- [ ] `.kiro/specs/cluster-*` の requirements / design / tasks を参照または新規作成する
- [ ] terminology reconciliation（CONTEXT.md 照合）を tasks 完了前に実施する

## 6. 実装規約

- [ ] 1 公開型 = 1 ファイル、`mod.rs` 禁止、sibling `*_test.rs`
- [ ] core crate に `#[cfg(feature = "std")]` を入れない
- [ ] adaptor が core concrete API を wrap しない（port 実装のみ）
- [ ] kernel → typed → adaptor の依存方向を維持する
- [ ] rustdoc は英語、通常コメントと Markdown は日本語

## 7. 検証

- [ ] 対象 crate の unit test / contract test を追加または更新する
- [ ] `./scripts/ci-check.sh ai all`（または変更範囲に応じた targeted check）を通す
- [ ] 利用例が必要な場合は [showcases/std](../../showcases/std/) に追加する（module 内 examples は作らない）

## 8. Parity 更新

- [ ] 実装完了後、`docs/gap-analysis/cluster-gap-analysis.md` の該当行を `実装済み` に更新する
- [ ] 必要に応じて `/pekko-gap-analysis cluster` を実行する
