# actor モジュール fake gap 分析

共通の定義と判断原則は [fake-gap-analysis-policy.md](/Users/j5ik2o/Sources/j5ik2o.github.com/j5ik2o/fraktor-rs/docs/gap-analysis/fake-gap-analysis-policy.md) を参照。

## サマリー

| 観点 | 判定 |
|------|------|
| 表面互換の進み具合 | 高い |
| fake gap の濃さ | 中程度 |
| 特に注意すべき領域 | signal, receptionist, delivery |

`actor` は `stream` ほど placeholder / no-op は多くない。
ただし typed の意味論を簡略化して parity っぽく見せている箇所が残っている。

## fake gap 一覧

| # | 領域 | fake gap | Pekko側 | fraktor-rs側 | 影響 | 重大度 |
|---|------|----------|----------|--------------|------|--------|
| 1 | typed signal | `Terminated` / `ChildFailed` の公開 signal 契約が縮退している | `Terminated(ref)`, `ChildFailed(ref, cause)` | `BehaviorSignal::Terminated(Pid)`, `ChildFailed { pid, error }` | signal 経由で参照そのものを扱う設計や、Pekko 準拠の public wrapper 拡張で詰まりやすい | medium |
| 2 | receptionist | 公開 API と内部実装が同居している | `typed/receptionist` 公開層 + `internal/receptionist` 内部層 | `receptionist.rs` が extension / behavior / state を同時に保持 | API は近くても、cluster receptionist や serializer 追加時に内部境界不足が露呈する | medium |
| 3 | delivery | public DTO と controller 実装 state machine が同じ層にある | `delivery/*` + `delivery/internal/*` | `typed/delivery/` 直下に command / settings / behavior / state が並列 | 今後の parity 拡張で、公開 API と内部進行制御が絡みやすい | medium |
| 4 | classic control surface | `PoisonPill` / `Kill` は実体はあるが、Pekko の public surface としては薄い | classic の制御メッセージ契約が明示的 | `SystemMessage::{PoisonPill, Kill}` と `ActorRef` helper 経由 | classic 互換レビュー時に「あるように見えるが同じ入口ではない」状態になる | low |
| 5 | behavior 階層 | `Behavior` はあるが `ExtensibleBehavior` のような段階的公開契約がない | `Behavior`, `ExtensibleBehavior` | `Behavior` 一枚に寄せている | interceptor / custom behavior の parity 議論で表面と内部の責務が混ざる | low |

## 詳細

### 1. typed signal の情報量縮退

Pekko では signal が public surface として独立しており、特に `Terminated` と `ChildFailed` は
「どの actor ref が落ちたか」を signal 型として保持する。

fraktor-rs では次のように `BehaviorSignal` enum へ畳み込んでいる。

- [behavior_signal.rs](/Users/j5ik2o/Sources/j5ik2o.github.com/j5ik2o/fraktor-rs/modules/actor/src/core/typed/message_and_signals/behavior_signal.rs)

この設計は Rust 的には簡潔だが、Pekko 互換として見ると
「signal の見た目は似ているが、公開契約は 1 段薄い」状態になっている。

### 2. Receptionist の公開 API / 内部実装の同居

`Receptionist` は公開 API として見せたい責務と、
実際の actor behavior / state machine / extension registration が一体化している。

- [receptionist.rs](/Users/j5ik2o/Sources/j5ik2o.github.com/j5ik2o/fraktor-rs/modules/actor/src/core/typed/receptionist.rs)

Pekko 側では public API と internal receptionist message / 実装がもう少し明確に分かれているため、
今のままだと「表面は Pekko に見えるが、内部は fraktor 独自の集約」に寄っている。

### 3. delivery の internal 層不足

`ProducerController` / `ConsumerController` は public surface としてはかなり揃っている。
しかし実装は DTO と state machine が同じ層にあり、Pekko の `delivery/internal` 相当の隔離がない。

- [producer_controller.rs](/Users/j5ik2o/Sources/j5ik2o.github.com/j5ik2o/fraktor-rs/modules/actor/src/core/typed/delivery/producer_controller.rs)
- [consumer_controller.rs](/Users/j5ik2o/Sources/j5ik2o.github.com/j5ik2o/fraktor-rs/modules/actor/src/core/typed/delivery/consumer_controller.rs)

この状態は「今は parity に見える」が、再送・永続キュー・監視連携の強化時に
公開 API の意味論と内部制御が密結合しやすい。

### 4. classic control surface の薄さ

`PoisonPill` / `Kill` は内部的には実装済みで、`ActorRef` helper もある。
ただし Pekko classic の public contract に対しては、fraktor-rs 側は
`SystemMessage` と helper の組み合わせで表現している。

これは fake gap としては軽いが、
classic parity を厳密に見ると「同じ名前・同じ入口」とまでは言えない。

## 改善策（破壊的変更前提）

### 1. typed signal を再設計する

理由:
- Pekko で signal が独立公開型なのは、ライフサイクル通知を単なる内部イベントではなく公開契約として扱うため
- 現在の `BehaviorSignal` への畳み込みは、利用者に見せるべき情報と内部配送都合を混在させている
- Rust でも、公開契約と内部配送表現を分けた方が型の責務が明確になる

- `BehaviorSignal` に情報を畳み込む現在の設計をやめる
- `Terminated`, `ChildFailed`, `PreRestart`, `PostStop` を独立した公開型として前面に出す
- `BehaviorSignal` は内部配送用 enum に縮小するか、完全に internal へ下げる

期待効果:
- Pekko と同じ粒度で signal 契約を扱える
- signal ごとの責務と拡張ポイントが明確になる

### 2. receptionist を `public API / protocol / internal implementation` に分割する

理由:
- Pekko 側で public と internal が分かれているのは、利用者に見せる契約と内部状態遷移を独立に進化させるため
- 現在の一枚構成では、公開 API の変更と内部実装の変更が同じ編集単位に閉じ込められ、責務が混ざる
- Rust でも `core` 内の責務分割を明示した方が、型の責務境界と依存方向を保ちやすい

- `receptionist.rs` の一枚構成をやめる
- `Receptionist` の公開 API
- `ReceptionistCommand` / `Listing` / `Registered` / `Deregistered` など protocol
- actor behavior / state / watch cleanup を担う internal implementation
  に分割する

期待効果:
- public API と内部状態遷移の境界が明確になる
- cluster receptionist や serializer 対応を足しやすくなる

### 3. delivery に `internal` 層を導入する

理由:
- Pekko の `delivery/internal` は、公開 DTO と進行制御 state machine を分離して、利用者契約を安定させるためにある
- 現在の fraktor 実装は controller の公開面と内部制御が同じ層にあり、実装の都合が公開設計へ漏れやすい
- Rust でも internal 層を切った方が、公開型を小さく保ちつつ実装詳細を差し替えやすい

- `ProducerController` / `ConsumerController` の state struct や deferred action を `delivery/internal/` へ移す
- `typed/delivery/` 直下には command / settings / DTO / public API だけを残す

期待効果:
- 公開契約と state machine 実装の混線を防げる
- parity を進める際に internal 実装だけ差し替えやすくなる

### 4. classic control surface を Pekko 寄りに揃える

理由:
- classic の `PoisonPill` / `Kill` は単なる内部制御メッセージではなく、利用者が直接使う public contract だから
- 現状の `SystemMessage` 依存表現では、内部実装の存在と公開契約が一致していない
- Rust でも public contract と internal message representation を分けた方が API の意味が明確になる

- `SystemMessage::{PoisonPill, Kill}` 依存の表現を利用者向け surface から隠す
- classic 側 public API に `PoisonPill` / `Kill` 相当の明示的な surface を出す
- helper ではなく契約として整理する

期待効果:
- classic parity の説明がしやすくなる
- 「内部実装はあるが public contract は違う」状態を解消できる

### 5. `Behavior` の公開階層を増やす

理由:
- Pekko で `Behavior` と `ExtensibleBehavior` が分かれているのは、利用者向け振る舞い契約と拡張ポイントを分離するため
- 現在の一枚構成では、単純利用者とカスタム拡張利用者が同じ抽象に乗っており、責務が曖昧になる
- Rust でも「誰が使う抽象か」を分けた方が公開面を説明しやすい

- `Behavior` 一枚構成をやめ、`ExtensibleBehavior` 相当を public に出す
- interceptor / custom behavior / DSL がどこにぶら下がるかを整理する

期待効果:
- Pekko typed の思考モデルに近づく
- 振る舞い拡張時の責務が分かりやすくなる

## 結論

`actor` は「かなり Pekko に近づいているが、内部はまだ fraktor 流の簡略化が残る」状態で、
完全な fake parity ではない。

本質的なズレは `signal`・`receptionist`・`delivery` に集中している。
破壊的変更を許容するなら、`signal 再設計 → receptionist 分割 → delivery internal 導入`
の順で進めるのが最も効果が高い。
