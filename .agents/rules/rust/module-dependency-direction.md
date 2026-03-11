# fraktor-rs モジュール依存方向ルール

## 原則

**依存は必ず上位層から下位層への一方向のみ。逆方向の依存を禁止する。**

## 層構造と依存方向

```
std 層         （std あり、std::typed / std::actor）
   │
   │  依存可（↓のみ）
   ▼
core/typed 層  （no_std、型付きアクター抽象）
   │
   │  依存可（↓のみ）
   ▼
core/actor 層  （no_std、untyped ランタイム基盤）
```

各層が「ラップする側」であり、「ラップされる側」には依存してよい。逆は禁止。

## 禁止パターン

| 禁止される依存 | 具体例 |
|----------------|--------|
| `core/actor` → `core/typed` | `ActorContext` が `TypedActorRef` を import |
| `core/actor` → `std` | untyped 基盤が std 機能に依存 |
| `core/typed` → `std` | typed コアが std 層に依存 |

```rust
// ❌ WRONG: core/actor/actor_context.rs
use crate::core::typed::actor::TypedActorRef;  // 禁止

// ✅ CORRECT: core/typed/actor/actor_context.rs
use crate::core::actor::ActorContext;  // typed → untyped は OK
```

## 判定フロー

```
1. 今いるファイルはどの層か？
   ├─ core/actor/** → 最下層。typed / std への use は禁止
   ├─ core/typed/** → 中間層。std への use は禁止
   └─ std/**        → 最上層。制限なし

2. 書こうとしている use が上の層を指しているか？
   ├─ Yes → 削除して代替手段を探す
   └─ No  → OK
```

## なぜこのルールが必要か

- `core/actor` は no_std ランタイム基盤。typed 抽象に依存すると循環参照・コンパイル不能になる
- `core/typed` は `PhantomData<M>` と `NonNull<ActorContext>` で型安全性を提供する薄いラッパー。std に依存すると no_std 環境で使えなくなる
- 依存方向が一方向であれば、下位層を上位層と独立してテスト・再利用できる

## 許容される例外

なし。循環依存が必要に見える場合は設計を見直すこと。
