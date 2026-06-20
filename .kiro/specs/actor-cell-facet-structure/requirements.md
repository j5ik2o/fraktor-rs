# 要件定義

## はじめに

`ActorCell` は actor-core-kernel の実行時コンテナとして、生成、dispatcher 接続、user/system message dispatch、supervision、children 管理、death watch、receive timeout、stash、timer、pipe task、adapter handle、lifecycle publication を一つの巨大ファイルで扱っている。`docs/gap-analysis/actor-gap-analysis.md` は、Pekko の `actor/dungeon/` が `Dispatch` / `FaultHandling` / `DeathWatch` / `Children` / `ReceiveTimeout` を分離している一方、fraktor-rs の `actor_cell.rs` は 1,809 行に複数責務が混在し、`actor_cell_test.rs` も 2,389 行のモノリスになっていることを構造ギャップとして記録している。

本仕様は、新しい actor runtime 機能を追加せず、`ActorCell` の観測可能な実行契約を保持したまま、保守者が責務単位で変更・検証できる facet 構造へ再編する。対象は actor-core-kernel 内部構造であり、利用者向け public API parity を増やす作業ではない。

## 境界コンテキスト

- **対象範囲**: `ActorCell` の生成・dispatch・lifecycle・fault handling・children・death watch・receive timeout・stash/timer/pipe/adapter handle に関する既存動作を保持しつつ、保守者が facet 単位でコードとテストを追える構造にすること。
- **対象外**: `SystemState` 分割、kernel public surface の棚卸し、typed 層の facade/behavior 分離、cell-level interceptor の新規公開、mailbox selection 契約、Replicator/cluster/stream/persistence 側の変更。
- **隣接システム／スペックへの期待**: 既存の `ActorContext`、`Mailbox`、`MessageDispatcherShared`、`ActorCellState`、`ReceiveTimeoutStateShared`、`ChildrenContainer`、`SystemStateShared` の公開・crate 内契約は、現在の挙動を維持したまま再利用できること。

## 要件

### 要件 1: ActorCell の観測可能動作の保持

**目的:** actor runtime 利用者として、内部構造が変わっても actor の実行結果が変わらないよう、`ActorCell` の既存実行契約を保持してほしい

#### 受け入れ基準

1. user message または system message の actor 配送が起きたとき、actor-core-kernel は再編前と同じ callback、mailbox scheduling、failure reporting を発生させなければならない。
2. actor の create、restart、stop、terminate が起きたとき、actor-core-kernel は再編前と同じ lifecycle event と guardian/system termination 結果を発生させなければならない。
3. actor invocation が失敗した場合、actor-core-kernel は再編前と同じ supervisor notification、mailbox suspension、children suspension、failure outcome を発生させなければならない。
4. actor が death watch の対象または watcher になった場合、actor-core-kernel は再編前と同じ watch/unwatch/terminated delivery と重複抑止を実行しなければならない。

### 要件 2: facet 単位で追跡できる責務境界

**目的:** 保守者として、ActorCell の変更箇所を責務単位で限定できるよう、dispatch、lifecycle、fault handling、children、death watch、receive timeout、stash、timer、pipe task、adapter handle を区別できる構造が欲しい

#### 受け入れ基準

1. 保守者による ActorCell の dispatch 経路変更が起きたとき、actor-core-kernel は user/system message invocation と mailbox pressure handling のコードを dispatch facet 内で追跡できるようにしなければならない。
2. 保守者による lifecycle または termination 経路変更が起きたとき、actor-core-kernel は create/stop/terminate/lifecycle publication のコードを lifecycle facet 内で追跡できるようにしなければならない。
3. 保守者による supervision failure 経路変更が起きたとき、actor-core-kernel は restart/resume/escalate/child failure decision のコードを fault handling facet 内で追跡できるようにしなければならない。
4. 保守者による child registry または death watch 経路変更が起きたとき、actor-core-kernel は children facet と death watch facet を分けて追跡できるようにしなければならない。
5. 保守者による receive timeout、stash、timer、pipe task、adapter handle の補助実行効果変更が起きたとき、actor-core-kernel は各責務を root ActorCell orchestration と相互の受け皿 module から分離して追跡できるようにしなければならない。

### 要件 3: 公開面と依存方向の非拡大

**目的:** crate 利用者として、内部リファクタリングによって public API や依存方向が広がらないよう、ActorCell の外部契約を変えないでほしい

#### 受け入れ基準

1. 再編後の actor-core-kernel は常に、`ActorCell` の既存 public method と crate 内 method の意味を維持しなければならない。
2. 再編後の actor-core-kernel は常に、新しい public facade、public trait、public helper type を追加せずに facet 構造を表現しなければならない。
3. 再編後の actor-core-kernel は常に、`*-core` の no_std 境界と既存の actor/dispatch/system 依存方向を維持しなければならない。
4. 再編後の actor-core-kernel は常に、直接 `std`、直接 `Arc`/`Mutex`、または project-defined sync abstraction を迂回する内部可変性を導入してはならない。

### 要件 4: テストと検証の facet 対応

**目的:** 保守者として、ActorCell の巨大テストを責務別に実行・診断できるよう、既存回帰を facet 単位で確認できてほしい

#### 受け入れ基準

1. ActorCell の children、death watch、fault handling、dispatch、receive timeout、stash、timer、pipe task、adapter handle の振る舞い検証が起きたとき、actor-core-kernel は該当 facet の sibling test で主要回帰を確認できるようにしなければならない。
2. 既存の ActorCell 回帰テストを移動または分割した場合、actor-core-kernel は再編前と同じ主要シナリオを失わずに保持しなければならない。
3. facet 再編完了が起きたとき、actor-core-kernel は targeted unit test、構造 lint、clippy、no_std check を通過しなければならない。
4. facet 再編完了が起きたとき、`actor_cell.rs` は常に root orchestration と module wiring に集中し、1,000 行を超える責務混在ファイルへ戻ってはならない。
