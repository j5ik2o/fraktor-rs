# 要件定義

## はじめに
actor-system-state-registry-split は、`SystemState` / `SystemStateShared` に集約されている actor system 内部状態を責務別 registry に分け、後続の EventBus、mailbox、CoordinatedShutdown 変更が無関係な state 領域へ波及しないようにするための構造整理である。対象は `actor-core-kernel` の system state 内部構造と既存 accessor の委譲であり、actor runtime の公開挙動は変えない。

## 境界コンテキスト（任意）
- **対象範囲**: `SystemState` / `SystemStateShared` の内部 registry 分離、既存 accessor の委譲化、shared wrapper と既存テストの維持。
- **対象外**: mailbox resolution の新仕様、汎用 EventBus trait 族、CoordinatedShutdown task variant、remote / serialization の公開挙動変更、typed system facade 分離、public re-export audit。
- **隣接システム／スペックへの期待**: `actor-eventbus-classification-contract`、`actor-mailbox-resolution-contract`、`actor-coordinated-shutdown-task-variants` は、この spec が安定化する registry 境界の上にそれぞれの新しい挙動を追加する。

## 要件

### 要件 1: 外部挙動の維持
**目的:** runtime 利用者として、構造整理後も既存 actor system API を同じ前提で使い続けるために、`SystemState` の registry 分離が公開挙動へ影響しないことが欲しい

#### 受け入れ基準
1. registry 分離が適用されたとき、actor-core-kernel は既存の actor spawn、path resolution、dispatcher lookup、mailbox creation、event stream publish、remote authority state、scheduler access の公開挙動を維持しなければならない
2. 既存 accessor が呼び出されたとき、actor-core-kernel は呼び出し元に新しい registry 型や内部所有構造を露出せず、既存の戻り値契約を維持しなければならない
3. 既存 typed facade が untyped system state を利用する間、actor-core-typed は event stream、scheduler、dispatchers、child lookup の観測結果を維持し続けなければならない

### 要件 2: subsystem registry 境界の明確化
**目的:** actor runtime 実装者として、後続の mailbox / EventBus / shutdown 変更を局所化するために、system state の状態所有境界を subsystem ごとに識別できることが欲しい

#### 受け入れ基準
1. SystemState の状態領域を変更するとき、actor-core-kernel は runtime support、dispatcher / mailbox、event / logging、guardian / cell table、serialization / remote hook、scheduler / shutdown coordination を区別できる registry 境界として表現しなければならない
2. mailbox または dispatcher に関する変更が起きたとき、actor-core-kernel は event、guardian、remote、scheduler の状態所有を変更せずに該当領域を更新できなければならない
3. EventBus または logging に関する変更が起きたとき、actor-core-kernel は mailbox、guardian、remote、scheduler の状態所有を変更せずに該当領域を更新できなければならない
4. guardian、cell table、actor path に関する変更が起きたとき、actor-core-kernel は remote authority、scheduler、event stream の状態所有を変更せずに該当領域を更新できなければならない

### 要件 3: shared state 操作の安全性
**目的:** actor runtime 実装者として、分割後も concurrent access の前提を壊さないために、shared wrapper の操作規約が維持されることが欲しい

#### 受け入れ基準
1. 分割された registry を共有所有する場合、actor-core-kernel は project-defined `Shared*` / `ArcShared` / closure-based access の規約を維持しなければならない
2. state 更新が複数の読み書きを伴う場合、actor-core-kernel は read-then-act の競合を増やさず、必要な更新を closure-based API の中で完結させなければならない
3. SystemStateShared が cached handle を返す間、actor-core-kernel は handle の同一性と clone 可能性を維持し続けなければならない
4. 直接の `Arc`、`Rc`、`std::sync::Mutex`、`spin::Mutex`、`spin::RwLock` を新規導入しようとした場合、actor-core-kernel は既存の project-defined shared abstraction を優先しなければならない

### 要件 4: no_std と依存方向の維持
**目的:** portable runtime 保守者として、core crate の移植性を維持するために、registry 分離が host runtime 依存を actor-core-kernel へ持ち込まないことが欲しい

#### 受け入れ基準
1. registry 分離が実装されたとき、actor-core-kernel は `no_std` + `alloc` の境界を維持しなければならない
2. scheduler、mailbox clock、remote hook、logging filter の状態を扱う間、actor-core-kernel は Tokio、std network I/O、host clock 実装への直接依存を追加してはならない
3. 新しい registry module を追加する場合、actor-core-kernel は既存の module wiring、1型1ファイル、sibling test 配置、FQCN import 回避の project lint 前提を満たさなければならない

### 要件 5: 検証可能な段階移行
**目的:** reviewer と実装者として、構造整理を安全にレビューするために、分割前後の同等性と後続 spec の接続点を確認できることが欲しい

#### 受け入れ基準
1. registry 分離が完了したとき、actor-core-kernel は SystemState / SystemStateShared の既存 unit test と関連 integration test を通過しなければならない
2. dispatcher / mailbox、event / logging、guardian / cell table、remote authority、scheduler / shutdown coordination の代表 accessor が使われたとき、actor-core-kernel は分割後 registry への委譲経路をテストで確認できなければならない
3. 後続 spec が mailbox、EventBus、CoordinatedShutdown の変更を開始するとき、実装者はこの spec の registry 境界を変更要否の判断材料として参照できなければならない
