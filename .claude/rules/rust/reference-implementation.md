# fraktor-rs 参照実装からの逆輸入手順

## 原則

**protoactor-go と Apache Pekko を参照しつつ、Rust の所有権と no_std 制約に合わせた最小 API を優先する。**

## 参照実装の位置

| 実装 | パス | 言語 |
|------|------|------|
| protoactor-go | `references/protoactor-go/` | Go |
| Apache Pekko | `references/pekko/` | Scala/Java |

## 逆輸入ワークフロー

```
1. 概念の特定
   対象機能に対応する参照実装のソースを特定する
   ├─ protoactor-go: Go のチャネル・goroutine ベースの設計
   └─ pekko: Scala の trait 階層・型クラスベースの設計

2. 型数の比較
   参照実装の公開型数を数え、fraktor-rs が同等以上に多い場合は過剰設計を疑う
   目安: fraktor-rs の公開型数 ≤ 参照実装の 1.5 倍

3. Rust イディオムへの変換
   ├─ Go goroutine + channel → async + mailbox
   ├─ Go interface{} → Rust の型パラメータまたは dyn Trait
   ├─ Scala trait 階層 → Rust trait + 合成（継承より合成）
   ├─ Scala implicit → Rust ジェネリクス + RuntimeToolbox
   └─ Scala Actor DSL → Rust Behavior パターン

4. no_std 制約の適用
   ├─ ヒープ割り当て → ArcShared / heapless を検討
   ├─ std 依存 → std モジュールに隔離
   └─ スレッド → ToolboxMutex で抽象化

5. 最小 API の原則
   ├─ 参照実装の全機能を移植しない
   ├─ 現在の要件で必要な機能のみ
   └─ YAGNI: 使われていない機能は作らない
```

## 変換時の注意点

### Go → Rust

| Go パターン | Rust パターン |
|-------------|--------------|
| `interface{}` | `dyn Any` / 型パラメータ `T` |
| `func(ctx Context)` | `&mut self` メソッド |
| `go func()` | `spawn` / async task |
| `chan T` | mailbox / mpsc channel |
| `sync.Mutex` | `ToolboxMutex` |
| `struct embedding` | trait 実装 + 委譲 |

### Scala/Pekko → Rust

| Pekko パターン | Rust パターン |
|----------------|--------------|
| `trait Actor` | `BehaviorGeneric<TB, M>` |
| `ActorRef[T]` | `TypedActorRefGeneric<TB, M>` |
| `Props` | `PropsGeneric<TB>` |
| `Supervision Strategy` | `SupervisorStrategyGeneric<TB>` |
| `implicit ActorSystem` | `TB: RuntimeToolbox` パラメータ |
| `sealed trait` + case classes | `enum` |
| `akka.pattern.ask` | `ask` Future |

## 比較レビューの実施タイミング

- 新機能の設計開始時
- 型の過剰設計が疑われるとき（`reviewing-fraktor-types` スキルと併用）
- 命名に迷ったとき（参照実装の命名を確認）

## 禁止パターン

- 参照実装の設計をそのまま移植（言語特性の違いを無視）
- 「pekko にあるから」という理由だけで型や機能を追加（YAGNI）
- 参照実装を読まずに独自設計を進める（先行事例の無視）
