## ADDED Requirements

### Requirement: `RuntimeMutex` / `RuntimeRwLock` は driver 差し替え可能な port surface を提供する

`fraktor-utils-core-rs` は、`RuntimeMutex` / `RuntimeRwLock` を単なる alias ではなく、driver 差し替え可能な port surface として提供しなければならない（MUST）。caller は lock 実装を直接固定せず、port 契約と factory seam を通じて driver を選択できなければならない（MUST）。

#### Scenario: utils-core は lock driver 契約を公開する
- **WHEN** `modules/utils-core/src/core/sync/` の公開面を確認する
- **THEN** `LockDriver` と `RwLockDriver` に相当する port 契約が存在する
- **AND** `RuntimeMutex` / `RuntimeRwLock` はその契約を使って driver 差し替え可能な surface として定義されている
- **AND** `RuntimeMutex<T> = SpinSyncMutex<T>` のような純 alias 定義だけには留まらない
- **AND** guard 型は trait contract の一部として表現される
- **AND** caller は lock()/read()/write() の戻り値に poison 固有の型を要求されない

#### Scenario: `RuntimeMutex<T>` / `RuntimeRwLock<T>` 名は default-driver surface として維持される
- **WHEN** 既存 caller が `RuntimeMutex<T>` / `RuntimeRwLock<T>` を import する
- **THEN** その名前は引き続き解決できる
- **AND** 定義は旧 alias-to-SpinSync ではなく、新しい port surface の default-driver instantiation になっている
- **AND** `NoStdMutex<T>` は `RuntimeMutex<T>` に追従する

#### Scenario: no_std builtin driver は core 側に残る
- **WHEN** `utils-core` の lock 実装配置を確認する
- **THEN** `SpinSyncMutex` / `SpinSyncRwLock` は no_std builtin driver として core 側に存在する
- **AND** caller はこれらを port surface の既定 driver 候補として利用できる

#### Scenario: driver selection は factory seam 経由で行える
- **WHEN** caller 側の lock 選択方法を確認する
- **THEN** `LockDriverFactory` / `RwLockDriverFactory` に相当する seam が存在する
- **AND** caller は mutex 実体型を直書きするだけでなく factory 経由でも driver を選択できる

#### Scenario: public API は driver generic parameter を露出しない
- **WHEN** public actor API を確認する
- **THEN** `ActorSystem` や `ActorRef` の public surface は driver generic parameter を持たない
- **AND** driver family の選択は bootstrap / configurator 境界で固定される

### Requirement: std adapter は `DebugSpinSyncMutex` と `StdSyncMutex` を提供する

`fraktor-utils-adaptor-std-rs` は、std adapter driver として debug 用および std 用の lock 実装を提供しなければならない（MUST）。

#### Scenario: std adapter は debug driver を提供する
- **WHEN** `modules/utils-adaptor-std/src/` の公開面を確認する
- **THEN** `DebugSpinSyncMutex` / `DebugSpinSyncRwLock` に相当する debug driver が存在する
- **AND** 対応する factory も存在する

#### Scenario: std adapter は std driver を提供する
- **WHEN** `modules/utils-adaptor-std/src/` の公開面を確認する
- **THEN** `StdSyncMutex` / `StdSyncRwLock` に相当する std driver が存在する
- **AND** 対応する factory も存在する

#### Scenario: poison は caller 契約へ露出しない
- **WHEN** std driver の contract を確認する
- **THEN** `std::sync` 由来の poison は driver 実装側で吸収される
- **AND** caller は poison 固有の型や分岐を要求されない
