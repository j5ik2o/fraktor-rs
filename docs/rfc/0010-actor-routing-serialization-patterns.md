# RFC 0010: routing・serialization・patterns

| 項目 | 内容 |
|------|------|
| Status | As-built |
| 対象コード | `modules/actor-core-kernel/src/routing/`, `serialization/`, `pattern/` |
| 関連文書 | RFC 0002（mailbox 観測値）, RFC 0006（scheduler）, `CONTEXT.md`（Circuit Breaker / Actor Selection） |
| 最終照合日 | 2026-07-11 |

本 RFC は 3 つの周辺サブシステムの**概説**であり、各領域の深掘りが必要になった時点で個別 RFC へ分割する。

## 1. routing

### 1.1 規範仕様

- **PAT-1.** `RoutingLogic` は `select(&self, message, routees) -> &Routee` の契約を持ち、routees が空の場合は `Routee::NoRoutee` を返す（エラーにしない）。実装は 4 種:
  - `RoundRobinRoutingLogic` — `AtomicUsize` の fetch_add による巡回
  - `RandomRoutingLogic` — seed 付き xorshift64（seed 0 は 1 に補正、ロックフリー）
  - `SmallestMailboxRoutingLogic` — mailbox 観測値のスコアで最小を選ぶ（suspended は最下位）。同点は常に index 0 が勝つ hot-spot 挙動を Pekko と同様に意図的に維持
  - `ConsistentHashingRoutingLogic` — Pekko のハッシュリングではなく **rendezvous hashing (HRW)** を採用（`&self` でステートレスに実装するための意図的 divergence）。安定マッピング・最小 disruption・`ConsistentHashableEnvelope` 優先を契約として宣言
- **PAT-2.** `SmallestMailbox` の観測値（`ActorRefObservation`）はフィールドごとに別瞬間のアトミック読みであり、**TOCTOU レースを許容する**。routing はベストエフォート・ヒューリスティクスであり、Pekko `selectNext` も同種の race を持つことが実装コメントに宣言されている。
- **PAT-3.** `Broadcast(AnyMessage)` は RoutingLogic ではなくメッセージラッパーであり、Router はこれを受けると logic を使わず全 routee へ配送する。
- **PAT-4.** `Pool` は routee を自ら spawn / supervise し、`Group` は既存 actor の path を束ねる（routee を作らない）。`Listen` / `Deafen` は Pid ベースで冪等な購読管理であり、`gossip` は全宛先へ配送を継続しつつ最初のエラーのみ返す（first-error ポリシー）。

### 1.2 不変条件・Open Questions

- **INV-PAT-1**: routee 選択は routees 集合の外を指さない（空なら NoRoutee）。
- **INV-PAT-2**: RoundRobin / Random / ConsistentHashing は `&self` で並行安全である（アトミック / ステートレス実装により成立）。
- **OQ-PAT-1**: ConsistentHashing が HRW である点は Pekko と分布特性が異なる（リング + virtual nodes ではない）。remote / cluster 側でハッシュ互換を仮定する機能が現れた場合に問題にならないか。

## 2. serialization

### 2.1 規範仕様

- **PAT-5.** `SerializationRegistry` の解決順序は cache → bindings（TypeId → SerializerId）→ fallback の 3 段であり、解決結果はキャッシュされる。id に対応する serializer が実在しない場合はキャッシュを無効化して `NotSerializable` を返す。
- **PAT-6.** 同一 type_name を別 TypeId で bind しようとした場合は衝突エラー（`serializer_binding_collision`）にしなければならない（MUST）。
- **PAT-7.** builtin serializer は 9 種（id 1〜9: Null / Bool / I32 / String / Bytes / ByteString / SystemMessage / MessageContainer / MiscMessage）。
- **PAT-8.** `AsyncSerializer` はオプトイン能力であり、実装者は同期 `Serializer` も併せて実装しなければならない（MUST）。
- **PAT-9.** `ActorRefResolveCache` は容量固定（既定 1024）+ 世代しきい値の bounded cache であり、`/user/temp/<name>`（一時 actor）のパスはキャッシュしない。

### 2.2 Open Questions

- **OQ-PAT-2**: serialization 失敗の観測は `SerializationErrorEvent` + Dead Letter（`SerializationError`）の二系統がある。両者の使い分け（どちらが必ず出るか）は明文化されておらず、観測側の期待を固定する必要がある。

## 3. patterns

### 3.1 規範仕様

- **PAT-10.** ask: `ask_with_timeout` はタイムアウト 0 で即 `AskError::Timeout`、それ以外は scheduler に期限を登録して未完了なら Timeout を設定する。reply は一時 actor（reply_ref）経由で future に届く。typed 側は同じカーネルヘルパを型付きで包む（型不一致は値取得時に `TypedAskError::TypeMismatch`）。
- **PAT-11.** retry: `retry(attempts, delay_provider, delay_for, operation)` は最大 attempts 回試行し、全滅時は最後のエラーを返す。`attempts == 0` は panic（呼び出し側契約違反）。
- **PAT-12.** graceful_stop: `PoisonPill` を送った後、対象 cell の消滅を 1ms 間隔でポーリングし、期限超過で `AskError::Timeout`。対象が既に不在なら即成功。
- **PAT-13.** CircuitBreaker は 3 状態機械であり、遷移は次のとおり（MUST）:
  - `Closed → Open`: 連続失敗が `max_failures` に到達
  - `Open → HalfOpen`: `reset_timeout` 経過後の最初の `is_call_permitted` がプローブとして許可される
  - `HalfOpen → Closed`: プローブ成功／`HalfOpen → Open`: プローブ失敗
  - HalfOpen 中の追加呼び出しはすべて拒否（プローブは 1 回限り）
  拒否は `CircuitBreakerOpenError { remaining }` として残り時間つきで返る。時刻は `Clock` port（RFC 0009）から得る。

### 3.2 不変条件・Open Questions

- **INV-PAT-3**: HalfOpen で同時に許可されるプローブは高々 1 つ。
- **INV-PAT-4**: graceful_stop が Ok を返すのは対象 cell がレジストリから消えた後のみ。
- **OQ-PAT-3**: graceful_stop の 1ms ポーリングは Blocker 抽象を使わない busy-wait 系実装であり、no_std 環境での消費電力・スレッドモデルとの整合を確認したい。

形式化候補（Lean）: CircuitBreaker の 3 状態機械（INV-PAT-3 のプローブ一意性を含む）は、時刻を抽象パラメータ化すれば小さく完全にモデル化できる。rendezvous hashing の最小 disruption（追加 1/(n+1)・削除 1/n）は確率的性質だが、決定的な「安定マッピング」（同一入力同一出力）は純関数の定理として書ける。

## 4. 参照

- Pekko: `pekko.routing` / `Serialization` / `CircuitBreaker`
- RFC 0002 / 0006 / 0009
