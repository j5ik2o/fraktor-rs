# ActorCell facet は private sibling module に留める

`ActorCell` は単一の public な実行コンテナとして維持し、dispatch、lifecycle、fault handling、children、DeathWatch (死亡監視)、Receive Timeout (受信タイムアウト)、stash、timers、pipe tasks、adapter handles は同一型を実装する private sibling module へ分ける。public surface audit の前に public facet trait や delegate handler type を増やさず、actor 実行上の変更を責務単位でレビューできるようにするためである。

**Considered Options**

- public または crate-visible な facet trait: 単一実装のために抽象を増やし、公開面を広げるため不採用。
- delegate handler type: ownership と shared-state の再設計へ波及するため不採用。
- private sibling module と同一型 `impl ActorCell`: 挙動と public API を維持しつつ root file を orchestration と accessor へ縮小できるため採用。
