# モジュール依存方向ルール

## 原則

**依存は必ず上位層から下位層への一方向のみ。逆方向の依存を禁止する。**

これはレイヤードアーキテクチャの普遍的な原則であり、言語を問わず適用する。

## 層構造と依存方向

```
上位層（std / application）
   │
   │  依存可（↓のみ）
   ▼
中間層（typed / domain）
   │
   │  依存可（↓のみ）
   ▼
下位層（untyped / infrastructure / runtime）
```

各層は「ラップする側（上位）」であり、「ラップされる側（下位）」には依存してよい。逆は禁止。

## fraktor-rs での対応

| 層 | パス | 備考 |
|----|------|------|
| 上位 | `std/**` | std あり、ユーザー向けAPI |
| 中間 | `core/typed/**` | no_std、型付きアクター抽象 |
| 下位 | `core/actor/**` | no_std、untyped ランタイム基盤 |

## 禁止パターン

| 禁止される依存 | 具体例（fraktor-rs） |
|----------------|----------------------|
| 下位 → 中間 | `core/actor` が `core/typed` を import |
| 下位 → 上位 | `core/actor` が `std` を import |
| 中間 → 上位 | `core/typed` が `std` を import |

```rust
// ❌ WRONG: core/actor/actor_context.rs
use crate::core::typed::actor::TypedActorRef;  // 禁止

// ✅ CORRECT: core/typed/actor/actor_context.rs
use crate::core::actor::ActorContext;  // typed → untyped は OK
```

## 判定フロー

```
1. 今いるファイルはどの層か？
   ├─ 下位層（core/actor/**） → 中間・上位への import は禁止
   ├─ 中間層（core/typed/**） → 上位への import は禁止
   └─ 上位層（std/**）        → 制限なし

2. 書こうとしている import が上の層を指しているか？
   ├─ Yes → 削除して代替手段を探す
   └─ No  → OK
```

## なぜこのルールが必要か

- **循環依存の防止**: 下位層が上位層に依存すると循環参照が発生し、ビルドが不能になる
- **独立テスト可能性**: 下位層が上位層に依存しなければ、下位層を単独でテスト・再利用できる
- **環境制約の維持**: 下位層が std 依存の上位層を import すると、no_std 環境で使えなくなる

## 許容される例外

なし。循環依存が必要に見える場合は設計を見直すこと。
