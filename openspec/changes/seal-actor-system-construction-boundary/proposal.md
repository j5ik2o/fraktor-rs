## 背景

Issue [#1735](https://github.com/j5ik2o/fraktor-rs/issues/1735) は `ActorSystem::from_state` と
`ActorSystem::create_started_from_config` の caller 整理を求めている。しかし元の作業項目は
「テスト caller を置換して可視性を下げる」ことに寄っており、設計上の問題を十分に閉じていない。

本質的な問題は、`ActorSystem` の外側から `SystemStateShared` を合成し、guardian / extension /
provider / serialization extension の bootstrap を通らない actor system handle を作れてしまう点である。
これは Pekko / Proto.Actor の参照実装が採る「公開入口は guardian/config/setup から system を起動する」
構造とずれており、テストの都合が production の construction boundary を汚染している。

正式リリース前なので、既存テストや downstream crate を壊さないための互換層は残さない。正しい境界は
「`ActorSystem` は bootstrap 済みの runtime handle であり、外部 caller は内部 state を差し込めない」
という形に寄せる。

この change は test caller 数を減らすための可視性変更ではない。`from_state` と
`create_started_from_config` は、test-only method として存在すること自体が設計を歪めているため削除する。
それに伴うテストコードの広範な書き換えは妥当な移行コストとして扱い、互換 helper や別名 API で温存しない。

## 目的

- 外部 crate から `SystemStateShared` を渡して `ActorSystem` を生成できないようにする。
- no-op/test actor system も production と同じ bootstrap 経路を通す。
- actor-core-kernel 内部で必要な handle 再構成は crate-private な実装詳細に閉じる。
- typed actor system の公開 constructor も同じ bootstrap 境界に揃える。
- std test helper の名前を実体に合わせ、guardian-less shell という古い概念を残さない。
- `ActorSystemSetup::into_actor_system_config` の単体テストを復元する。

## 変更内容

### 1. `ActorSystem::from_state` を public API から撤廃する

`ActorSystem::from_state(SystemStateShared)` は削除する。`#[doc(hidden)] pub` や deprecated alias は残さない。

actor-core-kernel 内部で weak upgrade / actor selection / actor cell back-reference に必要な
`SystemStateShared` から `ActorSystem` への再ラップは、crate 内部の実装詳細としてだけ保持してよい。
外部 crate、downstream tests、integration tests から `SystemStateShared` を actor system に戻す入口は
提供しない。

### 2. `ActorSystem::create_started_from_config` を削除する

`create_started_from_config` は root を started に見せるだけで、guardian bootstrap と extension /
provider installation を通らない。これは「空の system が欲しい」というテスト都合の bypass なので削除する。

no-op guardian でよい caller は、正式な bootstrap 経路である `ActorSystem::create_with_noop_guardian` を使う。

### 3. typed actor system の construction surface を揃える

`TypedActorSystem` は typed guardian を持つ正式な public construction API として
`create_from_props` / `create_from_behavior_factory` に加え、`create_with_noop_guardian` と
`create_from_props_with_init` を提供する。

typed `create_from_props` は typed `create_from_props_with_init` に委譲する。typed
`create_from_props_with_init` は kernel の `ActorSystem::create_from_props_with_init` を使い、typed bootstrap
（system receptionist、actor-ref resolver、event stream facade）を欠落させない。

typed no-op system が必要な caller は、`ActorSystem::create_with_noop_guardian` を `TypedActorSystem::from_untyped`
で包むのではなく、`TypedActorSystem::create_with_noop_guardian` を使う。

### 4. actor-adaptor-std の test helper を bootstrap 経由へ作り直す

`actor-adaptor-std` の test helper は `TestTickDriver` と std mailbox clock を設定したうえで、
`ActorSystem::create_with_noop_guardian` を呼ぶ。

`new_empty_actor_system` / `new_empty_actor_system_with` という名前は guardian-less shell を示しており、今後の
実体と合わないため、破壊的に `create_noop_actor_system` / `create_noop_actor_system_with` へ置き換える。互換 alias は
残さない。

### 5. テストは synthetic system ではなく目的別 helper へ移す

`ActorSystem::from_state(SystemStateShared::new(SystemState::new()))` で「system が 1 個欲しい」だけのテストは
`create_noop_actor_system` へ移行する。

bare `SystemState` を本当に検証したいテストは、`ActorSystem` に包まず `SystemState` / `SystemStateShared`
単位で検証する。actor context や cell が必要なテストは、bootstrapped no-op system 上に pid / cell を
明示的に作る。

### 6. `ActorSystemSetup::into_actor_system_config` の単体テストを復元する

PR #1734 で消えた `setup.into_actor_system_config()` 相当のカバレッジを、integration test の副作用ではなく
`ActorSystemSetup` の単体テストとして追加する。

## Capability

### 追加

- **`actor-system-construction-boundary`**
  - 外部 caller は `SystemStateShared` から `ActorSystem` を生成できない。
  - public actor system constructor は guardian/config/setup を経由して bootstrap を完了する。
  - test helper も production と同じ bootstrap 経路を通り、bootstrap bypass を提供しない。

### 変更

- **`actor-test-driver-placement`**
  - std 依存の test helper は actor-adaptor-std 側に置くという既存方針を維持する。
  - 追加で、helper が actor-core の private construction seam に依存してはならないことを明文化する。

## 影響範囲

**影響を受ける主なコード:**

- `modules/actor-core-kernel/src/system/base.rs`
- `modules/actor-core-kernel/src/system/actor_system_weak.rs`
- `modules/actor-core-kernel/src/actor/actor_selection/selection.rs`
- `modules/actor-core-kernel/src/actor/actor_cell.rs`
- `modules/actor-core-kernel/src/system/base/tests.rs`
- `modules/actor-core-kernel/tests/fixtures/kernel_public_surface/*`
- `modules/actor-core-typed/src/system.rs`
- `modules/actor-core-typed/src/system/tests.rs`
- `modules/actor-adaptor-std/src/system/empty_system.rs`
- `modules/actor-adaptor-std/src/system.rs`
- `new_empty_actor_system` または `ActorSystem::from_state` に依存する workspace 内 tests

**公開 API への影響:**

- BREAKING: `ActorSystem::from_state` を削除する。
- BREAKING: `ActorSystem::create_started_from_config` を削除する。
- BREAKING: `fraktor_actor_adaptor_std_rs::system::new_empty_actor_system*` を削除し、
  `create_noop_actor_system*` に置き換える。
- ADDED: `TypedActorSystem::create_with_noop_guardian` を追加する。
- ADDED: `TypedActorSystem::create_from_props_with_init` を追加する。

**挙動への影響:**

- test helper で作る system は、no-op user guardian、system guardian、extension installation、
  provider installation、default serialization extension、started root state を持つ。
- guardian-less shell に依存していたテストは、lower-level `SystemState` 検証へ寄せるか、bootstrapped system
  上で明示的に cell を作る必要がある。
- `from_state` / `create_started_from_config` の test caller は大量に書き換える。caller 数の多さは
  互換 constructor を残す理由にはしない。

## 非対象

- `ActorSystem::state()` の全面撤廃。これは construction seam とは別の privileged runtime surface であり、
  stream / remote / cluster の production 経路に広く使われているため、別 change で扱う。
- `SystemStateShared` 自体の公開 API 整理。
- actor testkit / probe の新設。
- 後方互換の deprecated alias。
