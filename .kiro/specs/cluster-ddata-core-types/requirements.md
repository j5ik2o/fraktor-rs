# 要件定義

## はじめに

Distributed Data / CRDT は fraktor-rs で全面未実装であり、Phase 3 の Replicator runtime を実装する前提となる「データ型の語彙」が存在しない。CRDT のデータ型と merge 法則は Replicator がなくても純粋に定義・検証できる。本スペックは `cluster-core-kernel` に `ddata` モジュールを新設し、状態ベース収束 CRDT（CvRDT）の基底 SPI、基本 CRDT 型、型付き Key・自ノード識別、整合性レベルと補助 protocol の語彙型を、Replicator runtime から切り離した純粋なデータ型契約として先行定義する。merge の収束法則（結合・可換・冪等）は property test で機械的に検証する。命名は参照実装（Apache Pekko）の CRDT ドメイン用語をそのまま採用する。

## 境界コンテキスト

- **対象範囲**:
  - CRDT 基底 SPI（merge / delta / ノード除去プルーニングの trait 契約）
  - 基本 CRDT 型: `Flag` / `GCounter` / `PNCounter` / `PNCounterMap`
  - 型付き `Key` 階層と自ノード識別 `SelfUniqueAddress`
  - read / write 整合性レベルの語彙型
  - 補助 protocol 語彙型（`GetReplicaCount` / `ReplicaCount` / `FlushChanges` 相当）
  - merge 法則の property test
- **対象外**:
  - `Replicator` runtime / gossip 接続 / `ReplicatorSettings`（Phase 3）
  - `ORSet` / `ORMap` / `ORMultiMap` / `LWWRegister` / `LWWMap` / `VersionVector`（Phase 2 — dot / tombstone / clock semantics が重く別スペック）
  - `PNCounterMap` の observed-remove（並行更新で復活し得る conflict-free なキー削除。`VersionVector` / `ORMap` を要するため別スペックへ委譲）
  - `DurableStore` SPI / std adapter、typed `DistributedData` extension、CRDT の wire serialization
- **隣接システム／スペックへの期待**:
  - ノード除去プルーニングは、クラスタのノード一意識別（`UniqueAddress` 相当）を入力として受け取る。識別子型の所有はノード除去判断側にあり、本スペックはそれを参照するのみ。
  - membership 用の `VectorClock` とは概念・責務を混同しない（CRDT の version 管理には流用しない）。
  - 将来の CRDT payload は `cluster-message-serialization-contract` のパターンに後続スペックで接続する（本スペックでは serialization を持たない）。

## 要件

### 要件 1: CRDT 基底 SPI（merge 契約）
**目的:** CRDT 実装者として、状態ベース収束 CRDT を一様に扱うために、merge を中心とした基底契約が欲しい

#### 受け入れ基準
1. `ddata` モジュールは常に、自分自身の型へ収束する merge 操作を備えた `ReplicatedData` 相当の基底契約を公開しなければならない
2. 同じ型の2つの CRDT 値が与えられたとき、基底契約の merge は両者を収束させた新しい値を返さなければならない
3. merge または更新操作が呼ばれたとき、`ReplicatedData` は元の値を破壊的に変更せず、結果を新しい値として返さなければならない

### 要件 2: Delta CRDT SPI
**目的:** CRDT 実装者として、全状態ではなく差分を伝播できるように、delta 対応の基底契約が欲しい

#### 受け入れ基準
1. delta 対応 CRDT が変異操作を受けたとき、その CRDT は最後のリセット以降に蓄積した delta を取得可能にしなければならない
2. delta が与えられたとき、delta 対応 CRDT は delta を全状態へ統合した新しい値を返さなければならない
3. delta の取得後にリセットが要求されたとき、delta 対応 CRDT は蓄積 delta を空にした値を返さなければならない
4. 既存状態のないレプリカに delta が到達したとき、`ReplicatedDelta` 相当の契約は空の全状態（zero）から初期状態を構築できなければならない
5. delta 型が因果順序配送を要求する場合、`ddata` モジュールはその要求を表すマーカ契約を公開しなければならない

### 要件 3: ノード除去プルーニング SPI
**目的:** 運用者として、クラスタから除去されたノードの寄与を CRDT から安全に畳み込めるように、プルーニング契約が欲しい

#### 受け入れ基準
1. ノード単位の状態を保持する CRDT は常に、状態を寄与したノード集合を報告できなければならない
2. 除去対象ノードが指定されたとき、CRDT はそのノード由来の状態を保持しているかを判定しなければならない
3. 除去対象ノードと畳み込み先ノードが指定されたとき、CRDT は除去ノードの寄与を畳み込み先へ移した新しい値を返さなければならない
4. プルーニングが確定した場合、CRDT は除去済みノードの残存エントリを取り除いた新しい値を返さなければならない

### 要件 4: Flag CRDT
**目的:** 利用者として、一方向の真偽フラグを収束的に共有するために、`Flag` 型が欲しい

#### 受け入れ基準
1. `Flag` が初期化されたとき、`Flag` は無効（false）でなければならない
2. 有効化操作が呼ばれたとき、`Flag` は有効（true）の新しい値を返さなければならない
3. いずれか一方が有効な2つの `Flag` を merge したとき、結果は常に有効でなければならない
4. `Flag` は常にノード識別を必要とせず、ノード除去プルーニングの対象に含めてはならない

### 要件 5: GCounter CRDT
**目的:** 利用者として、増加のみのカウンタを収束的に共有するために、`GCounter` 型が欲しい

#### 受け入れ基準
1. 自ノード識別と非負の増分が与えられたとき、`GCounter` は自ノードのスロットに増分を加えた新しい値を返さなければならない
2. 負の増分が与えられた場合、`GCounter` は増分を拒否しなければならない
3. 値が要求されたとき、`GCounter` は全ノードスロットの合計を返さなければならない
4. 2つの `GCounter` を merge したとき、結果は各ノードスロットの最大値を取らなければならない
5. `GCounter` は常にノード除去プルーニング契約を満たさなければならない

### 要件 6: PNCounter CRDT
**目的:** 利用者として、増減可能なカウンタを収束的に共有するために、`PNCounter` 型が欲しい

#### 受け入れ基準
1. 自ノード識別と増分が与えられたとき、`PNCounter` は増加成分へ反映した新しい値を返さなければならない
2. 自ノード識別と減分が与えられたとき、`PNCounter` は減少成分へ反映した新しい値を返さなければならない
3. 値が要求されたとき、`PNCounter` は増加成分の合計から減少成分の合計を引いた値を返さなければならない
4. 2つの `PNCounter` を merge したとき、増加成分と減少成分をそれぞれ独立に merge しなければならない
5. `PNCounter` は常にノード除去プルーニング契約を満たさなければならない

### 要件 7: PNCounterMap CRDT
**目的:** 利用者として、キー単位の増減カウンタ群を収束的に共有するために、`PNCounterMap` 型が欲しい

#### 受け入れ基準
1. 自ノード識別・キー・増減量が与えられたとき、`PNCounterMap` は該当キーの `PNCounter` を更新した新しい値を返さなければならない
2. キーが指定されたとき、`PNCounterMap` はそのキーの現在値を返し、存在しない場合は値なしを返さなければならない
3. 2つの `PNCounterMap` を merge したとき、キー集合の和を取り、同一キーの値は `PNCounter` の merge 法則で結合しなければならない
4. `PNCounterMap` は常にノード除去プルーニング契約を満たさなければならない
5. `PNCounterMap` は observed-remove（並行更新で復活し得る conflict-free なキー削除）を提供してはならず、キー集合は grow-only として扱わなければならない

### 要件 8: Key 階層と SelfUniqueAddress
**目的:** 利用者として、型安全に CRDT 値を識別し、カウンタ更新時の自ノードを明示するために、`Key` と `SelfUniqueAddress` が欲しい

#### 受け入れ基準
1. `Key` が生成されたとき、`Key` は文字列 id と対象 CRDT 型を表す型パラメータ（phantom）を保持しなければならない
2. 同じ id を持つ2つの `Key` は、型に関わらず等価と判定されなければならない
3. `ddata` モジュールは常に `Flag` / `GCounter` / `PNCounter` / `PNCounterMap` それぞれに対応する具象 `Key` 型を公開しなければならない
4. ノード単位状態を持つ CRDT の更新操作が呼ばれるとき、その操作は自ノード識別を暗黙のグローバル状態ではなく `SelfUniqueAddress` の明示引数として受け取らなければならない

### 要件 9: read/write 整合性レベル語彙
**目的:** 利用者として、将来の Replicator 操作の整合性を指定できるように、整合性レベルの語彙型が欲しい

#### 受け入れ基準
1. `ddata` モジュールは常に read 整合性として Local / From(n) / Majority / MajorityPlus / All に相当する variant を公開しなければならない
2. `ddata` モジュールは常に write 整合性として Local / To(n) / Majority / MajorityPlus / All に相当する variant を公開しなければならない
3. Local 以外の整合性レベルが構築されるとき、各 variant はタイムアウトを保持しなければならない
4. Majority または MajorityPlus が構築されるとき、各 variant は最小定足数下限を保持し、MajorityPlus はさらに追加ノード数を保持しなければならない
5. 整合性レベルの語彙型は常に Replicator runtime に依存しない純粋な値型として定義されなければならない

### 要件 10: 補助 protocol 語彙型
**目的:** 利用者として、将来の Replicator protocol を語彙として先行定義するために、補助 protocol 型が欲しい

#### 受け入れ基準
1. `ddata` モジュールは常にレプリカ数を問い合わせる `GetReplicaCount` 相当の型を公開しなければならない
2. `ddata` モジュールは常に自ノードを含むレプリカ数を表す `ReplicaCount` 相当の型を公開しなければならない
3. `ddata` モジュールは常に購読者通知の即時フラッシュを表す `FlushChanges` 相当の型を公開しなければならない
4. 補助 protocol 型は常に Replicator runtime に依存しない純粋な語彙型として定義されなければならない

### 要件 11: merge 法則の property test
**目的:** CRDT 実装者として、収束性を保証するために、merge 法則が機械的に検証されることが欲しい

#### 受け入れ基準
1. 各基本 CRDT 型に対して、merge の可換則（`a.merge(b)` と `b.merge(a)` が等価）が property test で検証されなければならない
2. 各基本 CRDT 型に対して、merge の結合則が property test で検証されなければならない
3. 各基本 CRDT 型に対して、merge の冪等性（`a.merge(a)` が `a` と等価）が property test で検証されなければならない
4. delta 対応 CRDT に対して、`mergeDelta` の結果が対応する全状態の merge と一致することが property test で検証されなければならない

### 要件 12: ddata モジュール境界と no_std 制約
**目的:** 保守者として、core の移植性を保つために、`ddata` を no_std で完結させたい

#### 受け入れ基準
1. `ddata` モジュールが新設されたとき、`ddata` は `cluster-core-kernel` 内のモジュールとして配置されなければならない
2. `ddata` モジュールは常に no_std + alloc で完結し、std への直接依存を持ってはならない
3. CRDT 型・SPI・語彙型の命名は常に参照実装（Pekko）の CRDT ドメイン用語に従わなければならない
4. `ddata` モジュールは常に membership 用の `VectorClock` を CRDT の version 管理に流用してはならない
