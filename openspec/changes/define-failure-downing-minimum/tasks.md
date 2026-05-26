## 1. 境界確認

- [x] 1.1 既存の `FailureDetector` / `MembershipCoordinator` / `DowningProvider` の責務を `failure-downing-minimum` spec と照合する。
- [x] 1.2 suspect / unreachable observation と member departure input が混同されている箇所を洗い出す。
- [x] 1.3 `docs/plan/2026-05-25_cluster-grain-runtime-roadmap.md` から failure / downing boundary note へリンクする。

## 2. Core contract

- [x] 2.1 `DowningDecision::{Down, Keep, Defer}` を `downing_provider` 配下に追加する。
- [x] 2.2 `DowningInput::{ExplicitDown, FailureObservation}` を `downing_provider` 配下に追加する。
- [x] 2.3 explicit down command が `DowningInput::ExplicitDown` として downing decision boundary を通ってから departure input になることを固定する。
- [x] 2.4 `FailureObservation` を `DowningInput::FailureObservation` へ渡せる入力として最小表現する。
- [x] 2.5 `DowningProvider` trait に `decide(&mut self, input: &DowningInput)` 相当の method を追加する。
- [x] 2.6 keep / defer 相当の decision が active topology を削除しないことを固定する。

## 3. 契約カバレッジ

- [x] 3.1 suspect observation が即 departure にならないことを membership tests で確認する。
- [x] 3.2 recovered observation が downing decision なしに active member を保持することを確認する。
- [x] 3.3 down decision が Grain runtime の stale activation / PID cache invalidation input になることを確認する。
- [x] 3.4 SBR / reachability matrix / rebalance / remembered entities がこの change に入っていないことを確認する。

## 4. 検証

- [x] 4.1 `define-failure-downing-minimum` の OpenSpec validation を実行する。
- [x] 4.2 `cluster-core` の targeted failure / membership / downing tests を実行する。
- [x] 4.3 変更した Markdown / Rust files の formatting checks を実行する。
