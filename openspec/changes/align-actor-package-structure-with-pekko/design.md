## Context

`modules/actor/src` は `core` と `std` の層分離を持っているが、`core` の最上位 package は責務軸の package 群の中に `typed` だけが型付け軸として置かれている。そのため、Pekko classic 相当の untyped runtime と Pekko typed 相当の typed runtime を 1 段目で分離できていない。

また `core/typed` 直下には、typed primitive と、service discovery、pubsub、routing の責務語彙が同列に並んでいる。`receptionist_command`、`service_key`、`listing`、`topic_command`、`topic_stats`、`*_router_builder` は、Pekko 側ではそれぞれ `receptionist`、`pubsub`、routing 周辺 package に属するが、現在は root typed にフラットに露出している。

この変更では、`core/kernel` と `core/typed` の二軸を先に確立し、その上で `typed` を Pekko Typed 由来の責務語彙へ再分解する。`core/std` 分離は維持し、Pekko の package 構造をそのまま写すのではなく、fraktor の層構造の中で対応付ける。

## Goals / Non-Goals

**Goals:**
- `modules/actor/src/core` の最上位分類軸を `kernel` と `typed` に揃える
- `core/typed` 直下の発見・pubsub・routing 語彙を Pekko 対応 package へ移す
- `core/typed` root を typed primitive の公開面として読みやすくする
- import path、`mod` 配線、tests/examples の参照を新構造に合わせて破綻なく更新する
- 構造変更ごとに `./scripts/ci-check.sh ai dylint` を回し、module wiring と lint 破綻を早期検出する

**Non-Goals:**
- actor runtime の振る舞いを新機能として拡張すること
- Pekko の `scaladsl` / `javadsl` を Rust にそのまま複製すること
- 今回の変更だけで `std` 側に新しい façade を増やすこと
- package 再編とは無関係な actor runtime の挙動変更を同時に行うこと

## Decisions

### 1. `core` の最上位は `kernel` と `typed` に揃える
- 採用: `core` の一段目を `kernel` と `typed` に整理する
- 理由: 現状は `typed` だけが型付け軸、他は責務軸であり、分類軸が揃っていないため
- 代替案: 現在の `core/*` を維持したまま typed だけ package 再編する
- 不採用理由: 1 段目の軸がずれたままで、Pekko classic/typed との対応関係が曖昧なまま残る

### 2. `typed` は `receptionist`、`pubsub`、`routing` に責務分割する
- 採用: `service_key` / `listing` / `receptionist_command` は `typed/receptionist`、`topic*` は `typed/pubsub`、router builder 群は `typed/routing` に配置する
- 理由: それぞれ Pekko Typed の責務語彙と自然に対応し、root typed の雑多さを解消できるため
- 代替案: root typed に置いたまま再 export だけ整理する
- 不採用理由: file path と package path が一致せず、責務境界がコード構造に現れない

### 3. `typed` root には typed primitive だけを残す
- 採用: actor、behavior、message_adapter、props、scheduler、spawn_protocol、supervise などの typed 基盤のみを root に残す
- 理由: typed root を「typed runtime の土台」として読み取れる形にするため
- 代替案: 後方互換のため root から広く再 export する
- 不採用理由: 正式リリース前であり、破壊的変更を許容できる段階で package 境界を曖昧に残す理由がないため

### 4. 実装時は file edit ごとに `ai dylint` を実行する
- 採用: file move / mod wiring / import 更新のたびに `./scripts/ci-check.sh ai dylint` を実行する
- 理由: package 再編は module wiring ミスの検出が遅れると修正コストが跳ねるため
- 代替案: ある程度まとめて変更してから lint を実行する
- 不採用理由: 変更単位が大きくなり、どの編集で壊れたか追跡しづらくなるため

## Risks / Trade-offs

- [Risk] import path の破壊的変更で tests/examples が広範囲に壊れる → Mitigation: package 再編は `receptionist`、`pubsub`、`routing` の順に進め、各ファイル編集ごとに `./scripts/ci-check.sh ai dylint` を実行する
- [Risk] `core/kernel` 化の途中で `mod` 配線が中途半端になりビルド不能時間が長くなる → Mitigation: 最上位 `mod` 再編を最初に行い、その後は責務 package ごとに完結させる
- [Risk] `typed` root からの再 export 削減で利用側が大量修正になる → Mitigation: proposal で breaking change を明示し、tasks で import 更新と examples/tests 更新を独立タスクに分ける
- [Risk] package 名だけ Pekko 風にして責務が実質変わらない可能性がある → Mitigation: file move だけでなく root typed の公開面削減と package 経由参照への更新を完了条件に含める
