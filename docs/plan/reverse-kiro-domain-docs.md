# Kiro spec から domain docs への逆同期メモ

## 目的

merged 済み Kiro spec のうち、feature-local な requirements / design / tasks に閉じている語彙と設計判断を、repo 全体の正本である `CONTEXT.md` と `docs/adr/` へ逆同期する。`CONTEXT.md` は glossary に限定し、実装手順、型一覧、task checklist は移さない。

## 対象

- `.kiro/specs/actor-cell-facet-structure/*`
- `.kiro/specs/actor-system-state-registry-split/*`
- `modules/actor-core-kernel/src/actor/actor_cell*.rs`
- `modules/actor-core-kernel/src/system/state/*.rs`

## 正常系

1. Kiro spec の境界語彙を抽出する。
2. 対応する code / tests が存在することを確認する。
3. repo-wide に使うべき概念だけを `CONTEXT.md` へ追加する。
4. 後で「なぜこの構造なのか」と疑問になりやすい判断だけを ADR 化する。
5. feature spec は要求・設計・タスクの正本として残し、`CONTEXT.md` / ADR へ重複コピーしない。

## 異常系

- spec と code がずれている場合は、`CONTEXT.md` へ確定語彙として追加しない。
- 実装型名一覧に過ぎないものは、`CONTEXT.md` ではなく spec / code 側に残す。
- 既存 `CONTEXT.md` の `_Avoid_` と衝突する表現は採用しない。
- reversible な実装配置や単なるファイル移動は ADR にしない。

## 抽出結果

### `CONTEXT.md` へ反映した語彙

| 用語 | 根拠 | 反映理由 |
|------|------|----------|
| Actor Cell (アクターセル) | `actor-cell-facet-structure`、`actor_cell.rs` | Actor 実行の中心境界で、Actor / ActorRef / Mailbox と混同しやすい |
| Actor Cell Facet (アクターセルファセット) | `actor-cell-facet-structure`、`actor_cell_*.rs` | public API ではなく内部責務境界であることを固定する必要がある |
| Actor System State (アクターシステム状態) | `actor-system-state-registry-split`、`system_state.rs` | system-wide state façade と subsystem behavior の混同を避ける |
| System State Registry (システム状態レジストリ) | `actor-system-state-registry-split`、`system/state/*_registry.rs` | public registry handle ではない private ownership 境界として固定する |
| DeathWatch (死亡監視) | `actor-cell-facet-structure`、`actor_cell_death_watch.rs` | Lifecycle Event / child registry / failure detection と混同しやすい |
| Receive Timeout (受信タイムアウト) | `actor-cell-facet-structure`、`actor_cell_receive_timeout.rs` | scheduler timeout や mailbox idle と混同しやすい |

### ADR 化した判断

| ADR | 根拠 | 判断 |
|-----|------|------|
| `docs/adr/0002-actor-cell-private-facet-structure.md` | `actor-cell-facet-structure` design / research と `actor.rs` private module wiring | ActorCell は public type のまま、private sibling facet module に同一型 impl を分ける |
| `docs/adr/0003-system-state-private-registry-facade.md` | `actor-system-state-registry-split` design / research と `system/state/*_registry.rs` | SystemState は façade として残し、registry は private leaf にする |

### 除外した候補

| 候補 | 除外理由 |
|------|----------|
| 実行補助 registry / DispatchMailboxRegistry など個別 registry 名 | 具体的な実装境界名であり、`System State Registry` の下位例として code / spec に残せばよい |
| Cached Handle | 実装最適化寄りで、domain glossary に入れるほど repo-wide の概念ではない |
| Stash / Timer / PipeTask / AdapterHandle | ActorCell facet の下位責務として code に残す。必要になった時点で個別に glossary 化する |
| Private leaf registry / façade delegation | 一般的な設計パターンであり、fraktor-rs 固有のドメイン語彙ではない |
| ActorCell facet spec の phase | `spec.json` は `tasks-generated` のままだが code は実装済み。語彙反映の根拠は code と merged PR の実体に置く |

## 次の候補

- `actor-mailbox-resolution-contract`
- `actor-eventbus-classification-contract`
- `actor-coordinated-shutdown-task-variants`

これらは今回追加した `Actor System State (アクターシステム状態)` / `System State Registry (システム状態レジストリ)` の語彙に接続できるかを確認してから、追加の glossary / ADR 化を判断する。
