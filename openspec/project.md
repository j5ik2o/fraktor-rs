# Project Context

## Purpose
分散アクターベースのランタイムとユーティリティ群を Rust で開発し、組み込みからクラウドまで一貫したメッセージ駆動アプリケーション基盤を提供する。

## Tech Stack
- Rust (stable/nightly)
- portable-atomic / portable-atomic-util
- Tokio, async-trait, spin 等の非同期・同期実装補助クレート
- Dylint ベースの社内 lint 群

## Project Conventions

### Code Style
- `rustfmt` / `cargo fmt` に準拠し、`#![deny(...)]` を多数設定して静的にスタイル逸脱を防ぐ
- 公開 API には RustDoc を付与し、`missing_docs` を許容しない
- 指定の lint（`clippy` や社内 Dylint）を全パスすること

### Architecture Patterns
- アクターや同期プリミティブは `ArcShared` などの抽象レイヤーで統一し、バックエンド依存を `feature` で切り替える
- `protoactor-go` / `pekko` を参照しつつ、Rust イディオムに沿ったモジュール構成を採用
- `no_std` 前提で `alloc` を最小限に使用しつつ、必要に応じて `force-portable-arc` 等の feature でハードウェア制約に対応

### Testing Strategy
- `./scripts/ci-check.sh all` および `makers ci-check` が成功することを必須とする
- Dylint による独自 lint 群を含め、clippy を `-D warnings` で実行する
- サンプルコードが存在する場合は `cargo run --example ...` で実行確認し、夜ly機能を利用する場合は `feature` を明示

### Git Workflow
- main ブランチに対する PR 前に CI を完走させる
- 破壊的変更は `openspec` による提案・承認後に着手
- コミットは論理単位で分割し、日本語/英語どちらでも一貫性のあるメッセージを記載

## Domain Context
- 分散アクターシステム向けであり、同期原語 (`ArcShared`, `RcShared`, `StaticRefShared`) などを理解していること
- `unsize` や `force-portable-arc` feature によってビルド対象が変わるため、nightly/stable の両対応を意識する

## Important Constraints
- `#![deny(...)]` のポリシーにより多数の lint がエラー化されているため、例外的変更は原則不可
- `unsize` feature は nightly 専用のため、stable ターゲット向けタスクでは無効化する必要がある
- `ArcShared` など共有ポインタ関連で暗黙 coercion を利用する場合、portable-atomic-util 側の cfg も前提となる

## External Dependencies
- crates.io 上の OSS（portable-atomic、spin、tokio など）に依存
- GitHub 上の `protoactor-go`, `pekko` を参照して設計を調整
- 社内 CI パイプライン（`cargo-make`, Dylint ドライバ）が整備されている
