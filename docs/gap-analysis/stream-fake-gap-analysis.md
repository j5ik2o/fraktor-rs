# stream モジュール fake gap 分析

共通の定義と判断原則は [fake-gap-analysis-policy.md](/Users/j5ik2o/Sources/j5ik2o.github.com/j5ik2o/fraktor-rs/docs/gap-analysis/fake-gap-analysis-policy.md) を参照。

## サマリー

| 観点 | 判定 |
|------|------|
| 表面互換の進み具合 | 中程度 |
| fake gap の濃さ | 高い |
| 特に注意すべき領域 | bridge API, materialization, watch/monitor, side-channel operators |

## fake gap 一覧

| # | 領域 | fake gap | 実装箇所 | 影響 | 重大度 |
|---|------|----------|----------|------|--------|
| 1 | input transform | `Flow::contramap` が変換せず `self` を返す | [flow.rs](/Users/j5ik2o/Sources/j5ik2o.github.com/j5ik2o/fraktor-rs/modules/stream/src/core/dsl/flow.rs#L1411) | 名前はあるが意味論がない | high |
| 2 | cancel hook | `Flow::do_on_cancel` が callback を保持しない | [flow.rs](/Users/j5ik2o/Sources/j5ik2o.github.com/j5ik2o/fraktor-rs/modules/stream/src/core/dsl/flow.rs#L1438) | side effect 契約が空 | high |
| 3 | fan-out helper | `Flow::also_to_all` が sink 数を数えるだけ | [flow.rs](/Users/j5ik2o/Sources/j5ik2o.github.com/j5ik2o/fraktor-rs/modules/stream/src/core/dsl/flow.rs#L2812) | API は存在するが配線しない | high |
| 4 | actor watch | `Flow::watch` が no-op | [flow.rs](/Users/j5ik2o/Sources/j5ik2o.github.com/j5ik2o/fraktor-rs/modules/stream/src/core/dsl/flow.rs#L2917) | watch 系 API が見かけだけ | high |
| 5 | monitor 契約 | `monitor` / `monitor_mat` / `watch_termination_mat` が Pekko 契約ではなく独自簡略化 | [flow.rs](/Users/j5ik2o/Sources/j5ik2o.github.com/j5ik2o/fraktor-rs/modules/stream/src/core/dsl/flow.rs#L2923) | materialized contract が別物 | high |
| 6 | subscriber bridge | `Source::as_subscriber()` が `Sink::ignore()` を返す | [source.rs](/Users/j5ik2o/Sources/j5ik2o.github.com/j5ik2o/fraktor-rs/modules/stream/src/core/dsl/source.rs#L185) | bridge API が空洞 | high |
| 7 | materializer bridge | `Sink::from_materializer()` / `from_subscriber()` / `future_sink()` がすべて `ignore()` | [sink.rs](/Users/j5ik2o/Sources/j5ik2o.github.com/j5ik2o/fraktor-rs/modules/stream/src/core/dsl/sink.rs#L130) | surface だけ揃って意味がない | high |
| 8 | publisher bridge | `Sink::source()` / `into_publisher()` が `Source::empty()` | [sink.rs](/Users/j5ik2o/Sources/j5ik2o.github.com/j5ik2o/fraktor-rs/modules/stream/src/core/dsl/sink.rs#L193) | publisher / source bridge 契約が空 | high |
| 9 | pre-materialize | `Source::pre_materialize()` / `Sink::pre_materialize()` が bridge ではなく単なる複製 | [source.rs](/Users/j5ik2o/Sources/j5ik2o.github.com/j5ik2o/fraktor-rs/modules/stream/src/core/dsl/source.rs#L2564) [sink.rs](/Users/j5ik2o/Sources/j5ik2o.github.com/j5ik2o/fraktor-rs/modules/stream/src/core/dsl/sink.rs#L187) | materialization の意味論が別物 | high |

## 詳細

### 1. operator 名だけ存在して契約が空

特に問題なのは以下の 4 つ。

- `contramap`
- `do_on_cancel`
- `also_to_all`
- `watch`

これらは Pekko 互換に見える名前を持っているが、
実際には入力変換・cancel hook・fan-out・watch を行っていない。

これは fake gap として最も危険なタイプで、
「実装済みだと思って使うと静かに意味がズレる」。

### 2. monitor / watchTermination の独自簡略化

`monitor` / `monitor_mat` / `watch_termination_mat` は、
Pekko の監視や termination materialized value 契約をそのまま再現していない。

fraktor-rs 側では `(u64, Out)` や `StreamCompletion<()>` ベースの独自モデルへ簡略化しているため、
名前は近くても意味は同じではない。

### 3. bridge API の placeholder 化

`Source.as_subscriber`、`Sink.from_subscriber`、`Sink.future_sink`、`Sink.source`、`Sink.into_publisher` などは、
Pekko では stream bridge の重要 API だが、fraktor-rs では placeholder のまま残っている。

ここは fake gap の典型で、
「表面だけ埋めていて parity に見えるが、内部実装はまだ存在しない」。

### 4. preMaterialize の意味論ズレ

`pre_materialize` は存在するが、
Pekko の「先行 materialization して bridge endpoint を返す」契約ではなく、
fraktor-rs では単に graph / mat を複製して返すだけの近似に留まる。

これは API 名の互換性に対して、意味論がまだ揃っていない。

## 改善策（破壊的変更前提）

### 1. placeholder / fallback API を一度削除する

理由:
- Pekko 互換名が public にある時点で、利用者は「意味論も揃っている」と期待するため
- no-op / placeholder を残すと、未実装より悪く「実装済みだが契約が違う」誤認を生む
- Rust でも公開 API は契約の宣言そのものなので、嘘の surface は残さない方が自然

- no-op / placeholder のまま残っている API は、実装できるまで public surface から外す
- `ignore()` / `empty()` / `self` を返すだけの fallback は互換実装と見なさない
- 互換名だけ残すラッパーや alias は追加しない
- 対象:
  - `contramap`
  - `do_on_cancel`
  - `also_to_all`
  - `watch`
  - `as_subscriber`
  - `from_materializer`
  - `from_subscriber`
  - `future_sink`
  - `source`
  - `into_publisher`

期待効果:
- 「名前はあるが意味が違う」罠をなくせる
- parity 進捗を正しく測れる

### 2. materialization 契約を先に作り直す

理由:
- `pre_materialize` や `watchTermination` 系は bridge API の前提になる中心契約で、ここがズレると周辺 API 全体が fake になる
- 現在は materialized value の意味論が Pekko と違うため、周辺 API を足しても互換ではなく近似が増えるだけ
- Rust でも基礎契約を先に固めないと、後続 API がすべてアダプタ的な継ぎ足しになる

- `pre_materialize`
- `monitor`
- `monitor_mat`
- `watch_termination_mat`

を Pekko と同じ意味論で作り直す
- fraktor 独自の簡略 contract に寄せた互換名はやめる
- Pekko 契約を満たせない間は非公開に戻すか、別名の fraktor API に分離する

期待効果:
- 後続の bridge API を正しい materialized contract 上に載せられる
- DSL の見かけだけ揃っていて内部が別物、という状態を減らせる

### 3. bridge API 群を `std` アダプタ側へ寄せる

理由:
- bridge API は subscriber / publisher / materializer など実行基盤依存の責務を持ち、no_std core に置くには不自然
- Pekko でも these APIs are runtime-backed; fraktor でも core と std の責務境界に従った方が設計が自然
- placeholder を core に置くより、実装可能な層に責務を移した方が fake parity を防げる

- subscriber / publisher / materializer bridge は `core` の placeholder ではなく、
  `std` 側の明示的アダプタへ寄せる
- `core` は no_std で保てる graph / stage / materialization 契約だけを持つ
- `core` に互換の見かけだけ置いて `std` に丸投げするような薄い public wrapper は作らない

期待効果:
- `core` の責務が明確になる
- 実行基盤依存の API を本当に実装できる場所に集約できる

### 4. 互換名を守れない API は別名へ逃がさず削る

理由:
- 別名の独自 API へ逃がすと「Pekko parity を目指す surface」と「fraktor 独自 surface」が混ざり、判定が曖昧になる
- parity 対象モジュールでは、互換名を名乗る以上は同じ契約を負うべき
- Rust 的にも、意味が違うのに近い名前を増やすのは API 設計として弱い

- Pekko と同じ意味論で実装できない間は、互換名を使わない
- fraktor 独自の簡略 API を残したいなら、それは Pekko parity とは別の文脈で設計する
- parity 対象モジュールでは「とりあえず似た名前で置く」を禁止する

期待効果:
- 利用者が Pekko 契約を期待して誤用するのを防げる

### 5. `Flow` / `Source` / `Sink` の公開面を縮小してから戻す

理由:
- 現在の問題は「数が足りない」ことより「public にあるものの信頼度が低い」こと
- 公開面を絞れば、残った API は「本当に使ってよい契約」として扱える
- Rust では公開範囲最小化が基本なので、未成熟 API を一旦引っ込める判断と相性がよい

- public に残すのは「Pekko 契約を満たすもの」だけに絞る
- parity 未達のものは `experimental` として残すのではなく、一旦消す
- 再追加は本実装とテストが揃った時点に限定する

期待効果:
- 「見た目は同じ」の誤認を防げる
- 公開 API 数より契約の正確さを優先できる

## 結論

`stream` は fake gap がかなり明確に存在する。

本質的な問題は、bridge API と materialization 系に
「Pekko 互換の名前があるのに契約がない」箇所が残っていることです。
破壊的変更を許容するなら、`placeholder API を一度削除 → materialization 契約を再設計 → bridge API を本実装で戻す`
の順が最も健全です。
