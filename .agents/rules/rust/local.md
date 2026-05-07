---
paths:
  - "**/*.rs"
---
# Rust Rules (Project-specific)

## Project-specific patterns

このファイルは fraktor-rs 固有の Rust パターンの索引とする。詳細本文は以下を正とする。

| パターン | 正となるファイル |
|----------|------------------|
| `ArcShared<T>` / `SharedLock<T>` / `SharedRwLock<T>` / `SharedAccess` | `./immutability-policy.md` |
| `DefaultMutex<T>` / `DefaultRwLock<T>` を渡す標準初期化形 | `./immutability-policy.md` |
| `*Shared` / `*Handle` / サフィックスなしの使い分け | `./naming-conventions.md` |
| Pekko / protoactor-go からの命名と設計の逆輸入 | `./reference-implementation.md` |
| CI の実行範囲と並行実行禁止 | `../project.md` |

## Examples

迷ったら `./examples.md` を見る。
