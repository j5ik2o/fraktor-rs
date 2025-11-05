# add-typed-actor-layer レビュー（Codex）

## 指摘事項
1. **`TypedActorContext::spawn` の型制約が厳しすぎる** — `openspec/changes/add-typed-actor-layer/design.md:23` では子アクターを `TypedActorContext<M>::spawn<N>` するときに `N` と `M` の整合を `N: Into<M>` 等で縛る案が書かれていますが、Pekko Typed では親が子の `ActorRef<ChildCommand>` をそのまま保持し、親自身の `Behavior<M>` とは独立したプロトコルで通信します。ここに変換境界を置くと、典型的な親子連携や cluster-sharding の like-for-like プロトコルが組めなくなるため、`N` は自由型であるべきです。`TypedActorSystem Generic Boundary` で root guardian だけを `M` に固定しているので、Context の spawn にはこの制約を持ち込まない設計に更新してください。
2. **MessageAdapter 要件が「やりとり全体」に適用され過ぎている** — `openspec/changes/add-typed-actor-layer/specs/typed-actor-layer/spec.md:28-41` の要件文は「型の異なる TypedActorRef 間のやりとりを行い、直接互換性のないメッセージ型は変換またはラップされなければならない」と読めますが、実際には「自分に届くメッセージ」を自プロトコルに合わせるための仕組みであり、他アクターへ送る側は単に相手の `TypedActorRef<TheirCommand>` を使うだけで十分です。現状の書き方だと「異なる型の相手に直接 send することすら禁止」と解釈され、Pekko の通常パターン（親が `ChildCommand` を送る、router が下流にブロードキャスト等）が不可能になります。要件を「受信側が異なるプロトコルを取り込むときに Adapter が必要」であると明示し、送信側の自由度を残す方向に修正してください。

## 良い点
- Proposal/Spec/Design が連動しており、Typed Spawn API → SpawnOpts → Props 互換という流れが明快です。
- MessageAdapter のシナリオに Pekko のユースケース（Event→Command 変換）を引用しており、実装時の期待挙動がイメージしやすくなっています。
