# Design: pekko-dispatcher-alias-chain

## Pekko 参照実装の分解

`references/pekko/actor/src/main/scala/org/apache/pekko/dispatch/Dispatchers.scala` の該当セクションを意味単位で分解する。

### 責務 A: Alias chain resolution (`lookupConfigurator` L159-198)

```scala
private def lookupConfigurator(id: String, depth: Int): MessageDispatcherConfigurator = {
  if (depth > MaxDispatcherAliasDepth)
    throw new ConfigurationException(
      s"Didn't find a concrete dispatcher config after following $MaxDispatcherAliasDepth, " +
      s"is there a loop in your config? last looked for id was $id")
  dispatcherConfigurators.get(id) match {
    case null =>
      val newConfigurator: MessageDispatcherConfigurator =
        if (cachingConfig.hasPath(id)) {
          val valueAtPath = cachingConfig.getValue(id)
          valueAtPath.valueType() match {
            case ConfigValueType.STRING =>
              val actualId = valueAtPath.unwrapped().asInstanceOf[String]
              logger.debug("Dispatcher id [{}] is an alias, actual dispatcher will be [{}]", id, actualId)
              lookupConfigurator(actualId, depth + 1)
            case ConfigValueType.OBJECT => configuratorFrom(config(id))
            case unexpected => throw new ConfigurationException(...)
          }
        } else throw new ConfigurationException(s"Dispatcher [$id] not configured")
      dispatcherConfigurators.putIfAbsent(id, newConfigurator) match { ... }
    case existing => existing
  }
}
```

**契約**:
- id → id の連鎖を最大 20 深さ辿る
- 20 超で cycle (または非現実的に深い chain) として `ConfigurationException`
- 途中で configurator が見つかったらそれを返す
- `MaxDispatcherAliasDepth = 20` は Pekko hardcode 定数 (L146)

### 責務 B: HOCON dynamic loading (`configuratorFrom` L263-291)

```scala
private def configuratorFrom(cfg: Config): MessageDispatcherConfigurator = {
  cfg.getString("type") match {
    case "Dispatcher"          => new DispatcherConfigurator(cfg, prerequisites)
    case "BalancingDispatcher" => throw new IllegalArgumentException(...)
    case "PinnedDispatcher" => new PinnedDispatcherConfigurator(cfg, prerequisites)
    case fqn =>
      prerequisites.dynamicAccess.createInstanceFor[MessageDispatcherConfigurator](fqn, args).recover { ... }.get
  }
}
```

**scope outside 判定**:
- HOCON 前提 (`cfg.getString("type")` / `Config` 引数)
- JVM reflection (`DynamicAccess.createInstanceFor[T]`) による FQN → インスタンス生成
- fraktor-rs には HOCON パーサも reflection 機構も無く、導入コストが高い割に、`Dispatchers::register(id, typed_configurator)` で等価な責務を果たせる

## Decision 1: alias と entry を別 HashMap で保持する (Pekko の `dispatcherConfigurators` 単一 map との差異)

**Pekko**: `dispatcherConfigurators: ConcurrentHashMap[String, MessageDispatcherConfigurator]` の単一 map。id が config の `STRING` (= alias) か `OBJECT` (= 実 config) かは HOCON parse 時点で判定。

**fraktor-rs**: `entries: HashMap<String, ArcShared<Box<dyn MessageDispatcherConfigurator>>>` + `aliases: HashMap<String, String>` の 2 map に分離する。

### Rationale

- fraktor-rs には HOCON が無いため、register 時点で alias か entry か区別する **explicit API** が必要 (`register(id, configurator)` vs `register_alias(alias, target)`)
- 2 map 方式により、型レベルで alias と entry の混同を防ぐ (`register_alias(id, "some-target")` と `register(id, configurator)` は別 API)
- alias と entry が同 id で登録されるのは error (明示的に検知可能)

### Trade-off

- alias 優先順位の契約: 同じ id が alias と entry の両方に登録されていた場合の挙動を決める必要がある → **alias を優先**し、`resolve()` は alias chain を先に辿ってから `entries` を lookup する。ただし `register_alias(id, ...)` / `register(id, ...)` の両方が成功することを許さない (後述 Decision 2)

## Decision 2: 衝突チェックの強度を API ごとに分ける

**動機**: 同じ id が alias と entry の両方に存在すると `resolve()` の挙動が曖昧になるが、API によって衝突時の適切な挙動が異なる:

- `register()` (strict add): 意図した新規登録で衝突したら error (silently overwrite しない)
- `register_alias()` (strict add): 同上
- `register_or_update()` (last-writer-wins): builder pattern の infallible 合成要件があり、戻り値を Result にすると連鎖が崩れる。かつ、ユーザーが既存 alias と同 id を update したい場合 (例: `with_dispatcher_configurator("pekko.actor.default-dispatcher", custom)` で Pekko alias を実 entry に置き換える) の正当な usecase がある

### 実装

#### register (strict)

```rust
pub fn register(&mut self, id: impl Into<String>, configurator: ArcShared<...>)
  -> Result<(), DispatchersError>
{
  let id = id.into();
  if self.aliases.contains_key(&id) {
    return Err(DispatchersError::AliasConflictsWithEntry(id));
  }
  match self.entries.entry(id.clone()) {
    Entry::Occupied(_) => Err(DispatchersError::Duplicate(id)),
    Entry::Vacant(v) => { v.insert(configurator); Ok(()) }
  }
}
```

#### register_alias (strict)

```rust
pub fn register_alias(&mut self, alias: impl Into<String>, target: impl Into<String>)
  -> Result<(), DispatchersError>
{
  let alias = alias.into();
  if self.entries.contains_key(&alias) {
    return Err(DispatchersError::AliasConflictsWithEntry(alias));
  }
  match self.aliases.entry(alias.clone()) {
    Entry::Occupied(_) => Err(DispatchersError::Duplicate(alias)),
    Entry::Vacant(v) => { v.insert(target.into()); Ok(()) }
  }
}
```

#### register_or_update (lenient, last-writer-wins)

```rust
pub fn register_or_update(&mut self, id: impl Into<String>, configurator: ArcShared<...>) {
  let id = id.into();
  self.aliases.remove(&id);   // 既存 alias は wipe される (先勝ち禁止)
  self.entries.insert(id, configurator);
}
```

alias が wipe されるのは「id」に対するエントリが alias → entry に変わるだけで、alias が指していた target 側の entry には影響しない。

### Error の対称性

`register` と `register_alias` の衝突で同じ `AliasConflictsWithEntry` を返す。id の役割 (alias か entry か) は context から自明なため、error variant に兼用して OK。

### Trade-off

`register_or_update` を lenient にすると、ユーザーが気づかず alias を消してしまう可能性がある。しかし:

- bootstrap 時のみの呼び出し経路である (call-frequency contract で spawn / hot path から呼ばれないことが保証)
- 消えるのは alias entry そのもの (target 側の entry は無影響)
- Pekko alias (`pekko.actor.default-dispatcher`) を自分の custom configurator に置き換えたいユーザー意図を自然に汲む

alternative として `register_or_update` が Result を返す設計も検討したが、`with_dispatcher_configurator` builder の戻り値型が `Self` → `Result<Self, _>` に変わって呼び出し側の全改修が必要になり、コスト対効果が合わない。

## Decision 3: `MAX_ALIAS_DEPTH = 20` を const として公開する

**動機**: Pekko `MaxDispatcherAliasDepth = 20` を一字一句合わせる。ユーザー / テストコードからも値を参照できるようにする。

```rust
impl Dispatchers {
  /// Maximum alias chain depth before rejection.
  ///
  /// Matches Pekko `Dispatchers.MaxDispatcherAliasDepth` (`Dispatchers.scala:146`).
  pub const MAX_ALIAS_DEPTH: usize = 20;
}
```

### Alternative: `u8` ではなく `usize` を使う理由

- Pekko 側は `Int` (i32 相当) だが、fraktor-rs ではインデックス型として `usize` が自然
- `u8` で十分だが、型を細かく分けると呼び出し側で cast が必要になる

## Decision 4: resolve 経路を単一化し、既存 normalize_dispatcher_id を alias 登録に移行する

**動機**: 現状 `resolve()` は `Self::normalize_dispatcher_id(id)` で hardcoded の 2 Pekko id (`pekko.actor.default-dispatcher` / `pekko.actor.internal-dispatcher`) を `default` に書き換えている。これは alias chain の特殊ケースであり、本 change で統一できる。

### 実装方針

- `normalize_dispatcher_id()` 関数は **削除**
- `ensure_default_inline()` / `replace_default_inline()` / `ensure_default()` 内で alias を 2 件自動登録:
  - `pekko.actor.default-dispatcher` → `default`
  - `pekko.actor.internal-dispatcher` → `default`
- `resolve()` は `follow_alias_chain` → `entries` lookup の 2 段構成に単純化

### Trade-off

- 既存 API の微妙な挙動変化: `normalize_dispatcher_id(id)` を外部から呼んでいるコードがあれば影響 → grep で確認して migration
- alias 経由になることで alias chain のテストカバレッジも副次的に上がる (Pekko id の normalize 経路が alias chain の 1 段テストを兼ねる)

### 既存の `normalize_dispatcher_id()` 公開範囲

現状 `pub` だが本 change で削除する。削除前の参照状況を要確認 (下記 Risk 3 参照)。

## Decision 5: alias は同じ target を指しても OK、cycle は depth over で検知

**動機**: Pekko も cycle を「depth over」で検知している (`if (depth > MaxDispatcherAliasDepth) throw ...`)。明示的な cycle 検出 (visited set) は不要。

### 理由

- 20 深さの探索は O(20) なので HashSet 作成のオーバーヘッドより軽い
- cycle に強いが cycle 以外の「本当に 20 層以上の alias chain を書いた」ケースとの区別は不要 (エラーメッセージは同一で「loop or deep chain」と表現する)

### エラーメッセージ

Pekko 原文: `"Didn't find a concrete dispatcher config after following $MaxDispatcherAliasDepth, is there a loop in your config? last looked for id was $id"`

fraktor-rs 版 (英語 rustdoc と異なり Display 出力は error message 言語としてユーザー向け。CLAUDE.md の「ドキュメント言語: rustdoc → 英語、その他 → 日本語」は適用されない。既存 `DispatchersError::Unknown` / `Duplicate` も英語なので英語で統一する):

```rust
Self::AliasChainTooDeep { start, depth } => write!(
  f,
  "alias chain starting at `{start}` exceeded max depth {depth} (possible cycle or excessive aliasing)"
),
```

## Decision 6: `follow_alias_chain` の戻り値は `String` で allocate する

**動機**: Rust の借用規則上、`&str` を返そうとすると HashMap 内部の `String` を借りることになり、resolve() 中に他のメソッドが self を借用できなくなる (後続の `entries.get(...)` も `&self` で呼ぶため同時借用になる)。

### 代替案と却下理由

- **案 A**: `&str` を返す → 上記理由で却下 (仮に動いても resolve の arm で後続処理が入れにくい)
- **案 B**: `Cow<str>` を返す → alias chain 長が 0 の場合だけ Borrowed、それ以外は Owned。micro-opt だが複雑化する
- **案 C**: `String` で allocate → **採用**。alias 解決は spawn / bootstrap 経路のみ (hot path ではない、call-frequency contract で既に明示) のため allocate コストは許容

### 実装

```rust
fn follow_alias_chain(&self, id: &str) -> Result<String, DispatchersError> {
  let mut current = id.to_owned();
  for _ in 0..=Self::MAX_ALIAS_DEPTH {
    match self.aliases.get(&current) {
      Some(target) => current = target.clone(),
      None => return Ok(current),
    }
  }
  Err(DispatchersError::AliasChainTooDeep { start: id.to_owned(), depth: Self::MAX_ALIAS_DEPTH })
}
```

loop 回数は `0..=MAX_ALIAS_DEPTH` (= 21 回) で、20 step alias を辿って 21 回目で「還」が無ければ Ok、21 回目でも alias が続くなら Err。これは Pekko `depth > MaxDispatcherAliasDepth` 条件と等価 (Pekko は初期 depth=0 で入り `depth > 20` で reject、すなわち 20 step までは OK 21 step 目で reject)。

## Risks and mitigations

### Risk 1: 既存 `normalize_dispatcher_id()` の外部利用

`Dispatchers::normalize_dispatcher_id()` は `pub` で、削除するにあたり外部利用を調査する必要がある。

**Mitigation**: `rtk grep "normalize_dispatcher_id" --glob "*.rs"` で利用箇所を列挙。Phase 1 で実施、利用が多い場合は public API を保ったまま内部で alias table を参照する形にする (互換 wrapper)。

### Risk 2: 既存テストの breaking

`dispatchers/tests.rs` に `normalize_dispatcher_id` のテストが存在する可能性。alias 化に伴い、これらのテストは alias 登録後に resolve を通すテストへ書き換える。

**Mitigation**: Phase 2 で既存テストを読み替え、alias chain 経路で同じ挙動を担保する。

### Risk 3: alias と entry の同 id 登録を検知しきれない順序

現状 `register` → `register_alias` の順序で検知する設計だが、`register_or_update` も衝突チェックが必要。

**Mitigation**: `register_or_update` も同じ衝突チェックを通す (`register_alias_or_update` は本 change の scope 外、必要性ゼロ)。

### Risk 4: alias chain resolution の runtime cost

`resolve()` は spawn / bootstrap 経路限定 (既存 call-frequency contract) のため、alias chain の 1 回分 HashMap lookup + String clone が数回入っても hot path 影響はない。

**Mitigation**: 既存 `resolve_count` カウンタで spawn 経路外での利用が検知可能。特に追加対策は不要。

## Non-goals (再掲)

- HOCON パーサ導入
- `type = "..."` による dynamic configurator instantiation
- `DynamicAccess` 相当の reflection 機構
- `hasDispatcher` / `Dispatchers.config(id)` / `defaultDispatcherConfig` 合成
- alias の removal API (`unregister_alias`)

これらは将来必要になれば別 change で検討する。
