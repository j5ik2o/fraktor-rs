## 文脈

### 現状

現在の `ActorSystem` には、通常の lifecycle を迂回する construction seam が 2 つある。

- `ActorSystem::from_state(SystemStateShared)` は、任意の shared state を `ActorSystem` に包める。
- `ActorSystem::create_started_from_config(ActorSystemConfig)` は、state を作って root を started にするが、
  guardian 作成や extension / provider / serialization extension の bootstrap を行わない。

これらの seam は主に downstream crate の tests で使われている。一方、production で state wrapping が必要な箇所は
actor-core-kernel 内部の weak upgrade、actor selection、actor cell context creation に限られる。

### 参照設計

Pekko は `ActorSystem(...)` / `ActorSystem.create(...)` を guardian behavior、name、config、setup から作る。
typed testkit も raw internal state constructor を公開せず、testkit guardian を持つ実 actor system を作る。
Proto.Actor も `NewActorSystem` / `NewActorSystemWithConfig` で registry、root context、guardians、event stream、
extensions、metrics をまとめて初期化する。

fraktor-rs でも以下を原則にする。

> public な `ActorSystem` は bootstrap 済み runtime handle を表す。raw state wrapping は
> actor-core-kernel の実装詳細であり、test や downstream extension mechanism ではない。

### 本質的な判断

この change では、テストコードの書き換え量より construction boundary の正しさを優先する。
`from_state` と `create_started_from_config` は test-only method として存在すること自体が production design を
歪めている。したがって caller が多くても、互換 API、deprecated alias、test-only public helper として残さない。

テストは「invalid shell を作る」方向ではなく、bootstrapped no-op system を使うか、`SystemState` /
`SystemStateShared` の lower-level contract を直接検証する方向へ大量に書き換える。この書き換えは副作用ではなく、
設計を正すための change scope に含める。

## 目的 / 非対象

**目的:**

- `SystemStateShared` を直接受け取る、または直接合成する public construction path を削除する。
- no-op/test actor system を production と同じ bootstrap path に通す。
- actor-core-kernel 内部の handle 再構成は public API にせず維持する。
- typed actor system の public constructor surface を kernel の bootstrap 境界に揃える。
- std test helper の名前を実体に合わせる。
- external crate から削除 API を呼べないことを regression test で固定する。
- `ActorSystemSetup::into_actor_system_config` の単体テストを復元する。

**非対象:**

- `ActorSystem::state()` の削除や privileged runtime access 全体の再設計。
- actor testkit の新設。
- 旧 helper 名の deprecated alias。
- `create_from_props_with_init` の削除。これは `create_from_props`、`create_with_noop_guardian`、typed system
  construction が使う primary bootstrap function として残す。
- `TypedActorSystem::from_untyped` の削除。これは既存の bootstrapped untyped system を typed API で包む advanced
  wrapper であり、raw state construction seam ではない。

## 設計判断

### Decision 1: public `ActorSystem::from_state` は削除する

**選択:** `ActorSystem::from_state` は削除する。外部 crate から見える `ActorSystem` API に
`SystemStateShared` を受け取る constructor を残さない。

actor-core-kernel 内部には、必要最小限の crate-private constructor を置く。

```rust
impl ActorSystem {
  pub(crate) const fn from_system_state(state: SystemStateShared) -> Self {
    Self { state }
  }
}
```

`ActorSystemWeak::upgrade`、`ActorSelection::resolve_actor_ref`、`ActorCell::make_context`、
actor-core-kernel の内部テストだけがこの helper を使ってよい。外部 crate からは到達できないため、これは
construction seam ではなく runtime handle の内部再構成である。

**理由:**

- Rust では sibling module から `ActorSystem` の private field を直接作れないため、crate-private helper は必要。
- 旧名 `from_state` を残すと、可視性が下がっても test seam としての意図が残る。公開 API の削除を grep で
  検証しやすくするため、旧名は完全に退役する。

### Decision 2: `create_started_from_config` は削除する

**選択:** `ActorSystem::create_started_from_config` は削除し、代替 public API は追加しない。

no-op guardian の actor system が必要な caller は既存の正式経路を使う。

```rust
ActorSystem::create_with_noop_guardian(config)
```

**理由:**

- `create_started_from_config` は root started flag だけを立てるため、`resolve_actor_ref` などの bootstrapped 判定を
  すり抜ける shell を作れる。
- extension installer / provider installer / default serialization extension を実行しない system は、
  production の actor system と同じ意味論を持たない。
- 空に見える system が欲しい場合も、Pekko の `Behaviors.empty` / testkit guardian と同じく、実際には
  no-op guardian 付き system として起動する方が正しい。

### Decision 3: `TypedActorSystem` は typed bootstrap constructor を公開する

**選択:** `TypedActorSystem<M>` は以下の public construction API を持つ。

```rust
impl<M> TypedActorSystem<M>
where
  M: Send + Sync + 'static,
{
  pub fn create_from_props(guardian: &TypedProps<M>, config: ActorSystemConfig) -> Result<Self, SpawnError>;

  pub fn create_with_noop_guardian(config: ActorSystemConfig) -> Result<Self, SpawnError>;

  pub fn create_from_props_with_init<F>(
    guardian: &TypedProps<M>,
    config: ActorSystemConfig,
    configure: F,
  ) -> Result<Self, SpawnError>
  where
    F: FnOnce(&ActorSystem) -> Result<(), SpawnError>;
}
```

`create_from_props` は `create_from_props_with_init(..., |_| Ok(()))` へ委譲する。
`create_with_noop_guardian` は `Behaviors::ignore` から typed no-op guardian props を作り、
`create_from_props` へ委譲する。`create_from_behavior_factory` は既存どおり typed guardian props を作る
convenience constructor として残す。

typed `create_from_props_with_init` は kernel の `ActorSystem::create_from_props_with_init` を使う。kernel の
bootstrap callback 内では、typed runtime に必要な system receptionist を先に install し、その後に caller の
`configure` を実行する。kernel bootstrap 完了後に `ActorRefResolver` と event stream facade を設定し、
`TypedActorSystem` として返す。

**理由:**

- spec が public construction input として typed guardian を含める以上、typed 側にも明示的な constructor surface
  が必要である。
- typed no-op system を `ActorSystem::create_with_noop_guardian` + `TypedActorSystem::from_untyped` で作ると、
  system receptionist など typed bootstrap を意図せず欠落させられる。
- `create_from_props_with_init` は typed runtime が system top-level actors を追加するための primary bootstrap
  seam であり、削除対象の bypass ではない。

### Decision 4: std test helper は `create_noop_actor_system*` に置き換える

**選択:** `actor-adaptor-std::system` から次を公開する。

```rust
pub fn create_noop_actor_system() -> ActorSystem;
pub fn create_noop_actor_system_with<F>(configure: F) -> ActorSystem
where
  F: FnOnce(ActorSystemConfig) -> ActorSystemConfig;
```

内部実装は以下の順序にする。

1. `ActorSystemConfig::new(TestTickDriver::default())`
2. `with_mailbox_clock(std_monotonic_mailbox_clock())`
3. caller の `configure`
4. `ActorSystem::create_with_noop_guardian(config)`

旧 `new_empty_actor_system*` は削除する。

**理由:**

- helper の実体は「guardian-less empty shell」ではなく「no-op guardian 付き bootstrapped system」になる。
- 後方互換 alias を残すと、古い概念が再び test code に広がる。
- std 固有の `TestTickDriver` と mailbox clock は actor-adaptor-std に閉じ、actor-core-kernel の no_std 境界を守る。

### Decision 5: actor-core-kernel inline tests の helper も bootstrap 経由にする

**選択:** `modules/actor-core-kernel/src/system/base/tests.rs` の test-only helper は crate 内部 helper として
残してよいが、実装は `create_with_noop_guardian` 経由に変更する。

必要なら内部 helper 名も `new_noop` / `new_noop_with` へ合わせる。ただし actor-core-kernel inline tests の
dev-cycle 制約により、adaptor-std の helper へ直接依存させる必要はない。

**理由:**

- actor-core-kernel の inline tests は同一クレート二バージョン問題があるため、すべてを adaptor-std helper へ
  寄せるのは別問題になる。
- 重要なのは「test helper でも bootstrap bypass を使わない」ことであり、内部 `TestTickDriver` 重複の解消ではない。

### Decision 6: downstream tests は system 合成ではなく目的別 setup へ移す

**選択:** downstream test の移行方針を 3 種類に分ける。

1. **System handle が欲しいだけ**
   - `fraktor_actor_adaptor_std_rs::system::create_noop_actor_system()` に置換する。
2. **Config を変えたい**
   - `create_noop_actor_system_with(|config| ...)` に置換する。
3. **bare state / synthetic cell が必要**
   - actor system に包まず `SystemState` / `SystemStateShared` の単体テストに寄せる。
   - actor context が必要なら bootstrapped no-op system 上で `allocate_pid` と `ActorCell::create` を使い、
     hard-coded `Pid::new(1, 1)` に依存しない。

**理由:**

- `from_state(SystemState::new())` は大半が `ActorContext::new` に渡す system が欲しいという便宜だった。
- convenience を理由に invalid system を作ると、extension/provider/bootstrap まわりの regression をテストが
  見逃す。
- pid を hard-code するテストは no-op guardian の導入で衝突しやすいため、pid allocation を system に任せる。
- 大量の test rewrite はこの decision の前提であり、test-only constructor を残すよりも正しい移行コストである。

### Decision 7: public surface fixture で削除 API を compile-fail にする

**選択:** `modules/actor-core-kernel/tests/fixtures/kernel_public_surface/` に compile-fail fixture を追加する。

対象:

- external crate から `ActorSystem::from_state(...)` を呼べないこと。
- external crate から `ActorSystem::create_started_from_config(...)` を呼べないこと。

既存の `kernel_public_surface.rs` harness に追加し、error message に旧シンボル名が含まれることを確認する。

**理由:**

- visibility 低下や削除は source grep だけでなく external crate 境界で保証する必要がある。
- `#[doc(hidden)] pub` のような再導入を防げる。

### Decision 8: `ActorSystemSetup::into_actor_system_config` は単体で検証する

**選択:** `modules/actor-core-kernel/src/actor/setup/actor_system_setup/tests.rs` に unit tests を追加する。

検証する内容:

- `BootstrapSetup` の system name / remoting config / start time が config に反映される。
- runtime settings (tick driver, scheduler, extension installers, provider installer, dispatcher, mailbox,
  circuit breaker config) が `into_actor_system_config` 後も保持される。
- `with_bootstrap_setup` は bootstrap 部分だけ置換し、runtime settings を落とさない。

**理由:**

- helper 削除と同時に integration test だけに依存していた coverage を補う。
- `create_with_noop_guardian` 経由へ寄せた後も setup から config への変換契約を維持できる。

## 移行計画

1. actor-core-kernel の public constructor surface を先に縮める。
2. actor-core-typed の constructor surface を `create_from_props_with_init` / `create_with_noop_guardian` まで揃える。
3. actor-adaptor-std helper を `create_noop_actor_system*` へ置き換える。
4. downstream tests を `create_noop_actor_system*` または lower-level state tests へ移す。
5. public surface compile-fail fixture を追加する。
6. `ActorSystemSetup` unit tests を追加する。
7. grep / cargo test / CI で削除名が残っていないことを確認する。

## 検証

最小検証:

- `rg -n "ActorSystem::from_state|create_started_from_config|new_empty_actor_system" modules tests showcases`
  が source code 上でヒットしないこと。
- `rg -n "pub .*from_state|create_started_from_config" modules/actor-core-kernel/src/system/base.rs`
  がヒットしないこと。
- `cargo test -p fraktor-actor-core-kernel-rs kernel_public_surface`
- `cargo test -p fraktor-actor-core-kernel-rs actor_system_setup`
- `cargo test -p fraktor-actor-core-typed-rs system`
- `cargo test -p fraktor-actor-adaptor-std-rs`
- `cargo test -p fraktor-persistence-core-rs`
- `cargo test -p fraktor-stream-core-kernel-rs`
- 最終確認として `./scripts/ci-check.sh ai all`

## 検討した代替案

### Alternative A: `from_state` を `pub(crate)` に下げるだけ

Issue #1735 の元案に近い。外部 caller は消せるが、旧名が残り、テスト seam を内部 API として温存する。
本 change では「外部から見える construction seam を撤廃した」ことを明確にするため採用しない。

### Alternative B: `create_started_from_config` を `pub(crate)` に下げる

actor-core-kernel inline tests からは使い続けられるが、bootstrap bypass が残る。テストが production と同じ
初期化経路を通らない問題が解決しないため採用しない。

### Alternative C: `new_empty_actor_system*` 名を維持して実装だけ変える

call site churn は少ないが、名前が嘘になる。正式リリース前で後方互換性を守る必要はないため採用しない。

### Alternative D: `ActorSystem::state()` も同時に削除する

construction boundary と privileged runtime access の両方を一度に閉じられるが、stream / remote / cluster の
production integration に広く影響する。今回の問題は「外部 state から `ActorSystem` を作れる」ことであり、
`state()` の代替 port 設計は別 change に分ける。

### Alternative E: test caller 向けの互換 constructor を残す

テストの書き換え量は減るが、`from_state` / `create_started_from_config` が作っていた設計上の歪みを別名で
温存するだけになる。test-only method が production API surface を規定する構造を断つことが目的なので採用しない。
