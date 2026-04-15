# ExtensibleBehavior 実装計画

## 背景
- actor モジュールの Phase 1 残タスクは `ExtensibleBehavior` のみです。
- `PoisonPill` / `Kill` と public signal 群は前回までのバッチで対象外になりました。
- 今回は Pekko の継承モデルをそのまま持ち込まず、fraktor-rs の typed root / DSL に自然な形で翻訳します。

## 実装方針
1. `modules/actor-core/src/core/typed/extensible_behavior.rs` を新設し、typed root の公開 trait `ExtensibleBehavior<M>` を定義する
2. `modules/actor-core/src/core/typed.rs` で `ExtensibleBehavior` を re-export する
3. `modules/actor-core/src/core/typed/dsl/behaviors.rs` に `Behaviors::from_extensible` を追加する
4. 既存の先行テストを成立させ、`AbstractBehavior` との共存を確認する

## 採否
| 項目 | 判定 | 理由 |
|------|------|------|
| `ExtensibleBehavior` trait 新設 | 採用 | typed root の公開拡張点が未実装のため |
| `Behaviors::from_extensible` factory | 採用 | `Behavior` の constructor visibility を広げずに runtime 接続できるため |
| `AbstractBehavior` の rename / 置換 | 非採用 | DSL 契約のすり替えになり、今回バッチの範囲を超えるため |
| `Behavior` の public constructor 化 | 非採用 | public/internal 境界を悪化させるため |

## 変更対象
- `modules/actor-core/src/core/typed/extensible_behavior.rs`
- `modules/actor-core/src/core/typed.rs`
- `modules/actor-core/src/core/typed/dsl/behaviors.rs`
- `modules/actor-core/src/core/typed/dsl/abstract_behavior/tests.rs`
- `modules/actor-core/src/core/typed/tests.rs`

## 非対象
- `NonBlockingBoundedMailbox`
- `VirtualThreadExecutorConfigurator`
- `LoggingFilter`
- classic routing の medium / hard 項目
- `AbstractBehavior` の全面統合
