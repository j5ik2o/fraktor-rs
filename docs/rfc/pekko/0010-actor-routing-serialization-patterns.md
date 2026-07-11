# RFC pekko-0010: routing・serialization・patterns（Pekko）

| 項目 | 内容 |
|------|------|
| Status | As-built (reference) |
| 対象コード | `references/pekko/actor/src/main/scala/org/apache/pekko/routing/`, `serialization/`, `pattern/`（+ `remote/.../serialization/ActorRefResolveCache.scala`） |
| 照合コミット | `references/pekko` @ `2dc8960074` |
| 対応 fraktor RFC | [0010](../0010-actor-routing-serialization-patterns.md) |
| 最終照合日 | 2026-07-11 |

## 1. routing

- **PPAT-1.** `RoutingLogic` 実装は **7 種**: RoundRobin / Random / SmallestMailbox / Broadcast / ScatterGatherFirstCompleted / TailChopping / ConsistentHashing（Broadcast も RoutingLogic として実装される）。
- **PPAT-2.** ConsistentHashing は **`SortedMap` ベースのハッシュリング + `virtualNodesFactor`（仮想ノード）**方式であり、ハッシュは MurmurHash、解決はフラット化した配列のバイナリサーチで行う。
- **PPAT-3.** SmallestMailbox のスコア優先順位は「NoRoutee > suspended > サイズ不明・処理中 > サイズ不明・待機 > サイズ既知 > メッセージなし」の昇順選択で、`hasMessages` と `numberOfMessages` の間の race は許容と実装コメントに明記されている（「Race between hasMessages and numberOfMessages here」）。
- **PPAT-4.** `Pool` は routee を子として生成し（既定 supervisor は全 Escalate）、`Group` は既存 actor を `ActorSelection` 経由で束ねる（watch しない）。`Resizer` は `isTimeForResize` / `resize` の 2 メソッドで pool を動的伸縮させるプラグイン点である。

## 2. serialization

- **PPAT-5.** バインディングは設定（`serialization-bindings`: クラス → serializer 名）で行い、既定で `allow-java-serialization = off`（Java シリアライズは `DisabledJavaSerializer` に置換）。プリミティブ・ByteString・バイト配列の専用 serializer が既定登録される。
- **PPAT-6.** `Serialization` extension が `serialize` / `deserialize` / `serializerFor` を提供する。ActorRef のパス解決キャッシュは `LruBoundedCache`（capacity 1024 / age threshold 600）を **ThreadLocal に 1 インスタンスずつ**持つ方式で、temp（ask 用一時 actor）パスはキャッシュしない。なおこの実装は `remote` モジュール側にある。

## 3. patterns

- **PPAT-7.** ask は `/temp` 配下の `PromiseActorRef` で reply を受け、タイムアウトは `AskTimeoutException`。
- **PPAT-8.** `gracefulStop(target, timeout, stopMessage = PoisonPill)` は「watch を張ってから stopMessage を tell し、`Terminated` の受信で成功」という **DeathWatch ベース**の実装である（ポーリングではない）。stopMessage は差し替え可能。
- **PPAT-9.** `retry` は attempts / 固定 delay / 遅延関数 / 指数バックオフのオーバーロードを持ち、`shouldRetry` の既定は「例外時のみ再試行」。
- **PPAT-10.** CircuitBreaker は Closed / Open / HalfOpen の 3 状態で、**`callTimeout` を持つ**（呼び出しがこれを超えると例外でなくても失敗と数える）。HalfOpen は単一プローブ（最初の 1 呼び出しのみ通し、並行呼び出しは fail-fast）。`resetTimeout` は `exponentialBackoffFactor` / `randomFactor` / `maxResetTimeout` により指数伸長できる（既定は無効化相当）。状態は Atomic 継承オブジェクトによる lock-free 実装。

## 4. fraktor-rs との差分

| 観点 | Pekko | fraktor-rs |
|------|-------|-----------|
| RoutingLogic の数 | 7（ScatterGather / TailChopping あり、Broadcast も logic） | 4 + Broadcast はメッセージラッパー（ScatterGather / TailChopping 相当は未実装） |
| ConsistentHashing | リング + 仮想ノード（MurmurHash） | **rendezvous hashing (HRW)**（意図的 divergence。fraktor RFC 0010 OQ-PAT-1 のとおり分布・互換性が異なる） |
| serialization 構成 | 設定ファイル駆動 + Java シリアライズの明示無効化 | コード登録（SerializationRegistry）+ builtin 9 種 |
| ActorRef 解決キャッシュ | ThreadLocal × LruBoundedCache(1024/600) | 所有権を持つ単一 `ActorRefResolveCache`（1024/600。パラメータは parity） |
| gracefulStop | DeathWatch ベース（Terminated 受信） | **1ms ポーリング**で cell 消滅を確認（fraktor RFC 0010 OQ-PAT-3 の裏付け——watch ベースへの置換が parity 候補） |
| CircuitBreaker | `callTimeout` あり、reset の指数バックオフあり | callTimeout なし・固定 resetTimeout（差分。導入するかは要判断） |
| SmallestMailbox の race 許容 / HalfOpen 単一プローブ / retry の形 | 同等 | 同等（parity） |

## 5. 参照

- fraktor 側 RFC 0010
- `ConsistentHash.scala:30-132`、`SmallestMailbox.scala:56-93`、`GracefulStopSupport.scala:59-69`、`CircuitBreaker.scala:114-193`、`reference.conf:792-838`
