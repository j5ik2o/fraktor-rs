## GitHub Issue #563: Epic: streams モジュール Pekko ギャップ対応

## 概要

streams モジュールの Pekko 比較ギャップを解消するためのEpicイシュー。

## 子イシュー一覧

| # | タイトル | 難易度 |
|---|---------|--------|
| #509 | OverflowStrategy 型定義の追加 (G-001) | trivial |
| #510 | SupervisionStrategy 公開化 + Decider 関数型の導入 (G-003, G-007) | easy〜medium |
| #511 | ActorSink の追加 — Source 側との対称性確保 (G-012) | medium |
| #512 | SourceQueue / BoundedSourceQueue + QueueOfferResult の追加 (G-004, G-009) | easy〜hard |
| #513 | TimerGraphStageLogic + AsyncCallback — ステージ非同期メカニズム (G-008, G-015) | medium〜hard |

## 対応順の目安

1. #509 OverflowStrategy（trivial、他の依存元）
2. #510 SupervisionStrategy 公開化（easy〜medium）
3. #511 ActorSink（medium）
4. #512 SourceQueue / BoundedSourceQueue（easy〜hard）
5. #513 TimerGraphStageLogic + AsyncCallback（medium〜hard）

### Labels
enhancement, streams