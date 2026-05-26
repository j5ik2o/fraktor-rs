## 背景

`cluster-core` にはすでに `FailureDetector`、`FailureDetectorRegistry`、`MembershipCoordinator`、`MembershipEvent::MarkedSuspect`、`CurrentClusterState::unreachable`、explicit `down` command で使われる `DowningProvider` hook がある。provider boundary change では、provider が topology または departure input を供給し、Grain runtime が provider-neutral な状態を消費する境界を固定した。

未整理なのは、failure observation から member departure へ進む間の責務境界である。現状の `suspect_timeout` は membership transition を進められるが、downing decision model として独立した contract にはなっていない。この境界が曖昧なままだと、後続の SBR、reachability matrix、rebalance の作業が provider や Grain runtime の semantics へ漏れやすい。

## 目的 / 対象外

**目的:**

- suspect / reachable / unreachable state に関する最小の failure observation contract を定義する。
- downing decision がどこで作られ、どのように member departure input になるかを定義する。
- `cluster-core` が decision port と membership state semantics を所有する形を維持する。
- std adapter は detector implementation、timer、networking、runtime execution の供給に留める。
- 既存の membership / failure detector / downing tests と contract を対応づけ、不足があれば小さい targeted test だけを追加する。

**対象外:**

- Split Brain Resolver behavior の実装。
- reachability matrix または full gossip reachability semantics の導入。
- rebalance、remembered entities、in-flight drain、recovery behavior の実装。
- local / static / AWS ECS provider への provider-specific failure policy 追加。
- Pekko public API parity としての cluster downing 定義。

## 決定事項

### Decision 1: failure-downing-minimum は新 capability として切る

Provider boundary spec は downing policy を明示的に対象外にしている。`failure-downing-minimum` capability を分けることで、provider discovery や Grain placement と混ぜずに failure observation と decision flow へ焦点を絞る。

代替案: `cluster-provider-boundary` に downing requirement を追加する。この場合、provider input と failure policy が混ざり、provider が downing decision を所有しているように見えやすい。

### Decision 2: suspect / unreachable は observation であり departure ではない

Failure detector と membership coordination は member を suspect または unreachable として扱えるが、その observation は member departure と同義ではない。Departure input は、explicit down command または downing decision が active topology から authority を外す段階で始まる。

代替案: suspect timeout を常に implicit downing として扱う。これは単純だが、policy boundary を隠し、後続の SBR や manual downing を導入しづらくする。

### Decision 3: DowningProvider は decision boundary である

`DowningProvider` は core-owned な downing behavior port として扱う。この change では、現行の explicit `down(authority)` hook だけでは不十分と判断し、failure observation も受け取って down / keep / defer 相当の decision を返せる contract へ拡張する。

最小 API shape は次の形にする。

- `DowningDecision` enum を `cluster-core` の `downing_provider` 配下に追加し、variant は `Down` / `Keep` / `Defer` に絞る。
- `FailureObservation` 型を `downing_provider` 配下に追加し、authority、observation kind、観測時刻など、既存 `MembershipEvent::MarkedSuspect` と `CurrentClusterState::unreachable` から構成できる最小情報だけを持たせる。
- `DowningProvider` trait に `decide(&mut self, observation: &FailureObservation) -> Result<DowningDecision, ClusterProviderError>` 相当の method を追加する。
- 既存の explicit `down(authority)` hook は explicit down command 用の入力として残し、内部的には `DowningDecision::Down` を許可する経路へ統合できるようにする。

代替案: 現行の explicit `down(authority)` hook だけを維持する。これは変更量を抑えられるが、failure observation から departure input へ進む policy boundary を表現できない。もう一つの代替案として `MembershipCoordinator` が downing decision を直接所有する案もあるが、failure detection、policy、topology mutation の結合が強くなりすぎる。

### Decision 4: Grain runtime は member departure input だけを消費する

Identity lookup、placement、activation、PID cache invalidation は provider-neutral topology と departure input を観測し続ける。phi value、suspect timer、SBR choice、detector-specific state は inspect しない。

代替案: Grain runtime が unreachable state を直接 inspect する。この場合、placement policy が failure detector details に依存し、membership semantics と重複する。

## リスク / トレードオフ

- [リスク] 最小 contract が将来の SBR には弱すぎる。 -> 緩和策: SBR は将来 capability として明示的に分離し、この change は port shape と state transition に留める。
- [リスク] `DowningProvider` 変更が早すぎる可能性がある。 -> 緩和策: 既存 tests との照合から始め、decision output を表現するために必要な最小 API だけを追加する。
- [リスク] suspect timeout behavior がすでに policy を含んでいる可能性がある。 -> 緩和策: code change の前に、現行 timeout が default downing strategy なのか coordinator transition なのかを文書化する。
- [リスク] provider boundary と downing boundary が重なる。 -> 緩和策: provider specs は discovery / topology input を所有し、この capability は failure observation と decision semantics だけを所有する。
