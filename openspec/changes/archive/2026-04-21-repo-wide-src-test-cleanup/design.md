## Context

`repo-wide-src-test-cleanup` は、panic-guard change の完了判定で露出した「repo 全体の test 配置と健全性条件のねじれ」を独立に解消するための設計である。現状は `src/` 配下に `#[cfg(test)] mod tests;` と `tests.rs` が散在し、no_std-sensitive な core crate でも std 依存 test が production source tree に混在している。

また、repo-wide `dead_code` 条件を厳密に適用すると、今回のような feature change と無関係な既存 test helper まで巻き込んで fail する。目的は公開 API を変えることではなく、production code と test-only code の境界を整理して、今後の change が repo-wide cleanup に足を取られない状態を作ることである。

## Goals / Non-Goals

**Goals:**
- `src/` 配下に残る std 依存 test module を洗い出し、`tests/` へ移せるものを段階的に移動する
- no_std-sensitive な crate では production path と test-only path の境界を明確にする
- test-only helper / type / method を production module から切り離し、repo-wide `dead_code` 適用に近づける
- 既存テストの意味論を保ったまま、配置と依存方向だけを健全化する

**Non-Goals:**
- panic-guard や他の feature change の runtime behavior を変更すること
- 公開 API の整理や命名変更を主目的にすること
- 一回の change で全 crate の `src` 内 test を完全撤去すること
- 新しい lint を追加して強制すること

## Decisions

### 1. cleanup は feature change から切り離し、独立 change として進める

- `2026-04-20-pekko-panic-guard` のような機能 change に repo-wide cleanup を混ぜると、変更意図と失敗原因の対応が崩れる
- cleanup を独立 change にすることで、目的を「境界整理」に限定できる

**Alternatives considered**
- panic-guard 側でそのまま repo-wide cleanup を実施する
  - 却下。優先順位が逆転し、影響範囲が不要に広がる

### 2. std 依存 test は `src` から `tests/` へ移し、production source tree から切り離す

- no_std-sensitive な crate では、production source tree の grep や lint で `std::*` が混ざるだけで誤検知の温床になる
- integration test へ移せる test は `tests/` 配下へ移し、production module には implementation だけを残す

**Alternatives considered**
- `#[cfg(test)]` のまま `src` に残し、ルール側だけ緩める
  - 却下。境界の曖昧さが残り、将来の change でも同じ議論を繰り返す

### 3. cleanup は batch 単位で進め、各 batch で同等テストの再実行まで含める

- repo-wide 一括移動は差分が大きく、失敗時の切り分けが困難になる
- crate / モジュール単位で候補を分け、各 batch で `cargo test` / `ci-check` を回す

**Alternatives considered**
- 全 `src/**/tests.rs` を機械的に一括移動する
  - 却下。private visibility や helper 依存のため、局所調整なしでは壊れやすい

### 4. `dead_code` 整理は production module の責務を崩さない範囲でだけ行う

- test-only helper が production path に置かれていること自体が問題なので、まず配置の整理を優先する
- `dead_code` 0 を目標にするが、公開 API や runtime logic を歪めてまで達成しない

**Alternatives considered**
- `#[allow(dead_code)]` を付与して即時解消する
  - 却下。ガイドライン違反であり、根本解決にならない

## Risks / Trade-offs

- [private helper の可視性が崩れる] → integration test へ移す際は helper を production 公開面に出さず、test 専用 builder / fixture に閉じる
- [差分が広くなり review しにくい] → crate / module ごとに batch 化し、各 batch で対象一覧を明示する
- [test runtime が変わる] → move 後も同じ assertion と実行コマンドを維持し、意味論の差分を入れない
- [repo-wide `dead_code` が一度で 0 にならない] → batch ごとに backlog を更新し、未着手箇所を明示して進める

## Migration Plan

1. `src/` 配下 test module の棚卸しを行い、std 依存 / no_std 影響 / private helper 依存を分類する
2. 移設コストの低い module から `tests/` へ移す
3. 同じ batch 内で不要 helper / method を整理する
4. 各 batch ごとに対象 crate の `cargo test` と `ci-check` を回す
5. 最終的に repo-wide `dead_code` / `src` grep 条件へ近づける

## Open Questions

- repo-wide `dead_code` を最終 acceptance に含めるタイミングをどこで切るか
- `src` 内 test を完全禁止するのか、それとも no_std-sensitive crate だけを先に対象化するのか
