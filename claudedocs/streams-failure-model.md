# Streams 障害モデルと supervision 適用範囲

## 目的
Pekko 互換強化における Phase 3 の設計判断を、現行 `modules/streams` 実装に即して明文化する。

## Failure と Error の扱い
1. **Failure（ストリーム失敗）**  
   実行器が受け取る `StreamError` はストリーム制御失敗として扱う。  
   代表例: `TypeMismatch`, `InvalidConnection`, `BufferOverflow`, `Failed`。
2. **Error（データとしてのエラー）**  
   `Result<T, StreamError>` を要素型に持つ場合、`Err` は要素データとして流せる。  
   `recover` / `recover_with_retries` はこの「エラーペイロード」を変換対象にする。
3. **整理方針**  
   - 制御失敗は従来どおり `StreamError` で停止させる。  
   - データ失敗は `Result` 要素で表現し、必要に応じて recover 系で復旧する。

## supervision 対応可否（現時点）
| 対象 | API | 現在の挙動 |
|------|-----|------------|
| Source | `supervision_stop/resume/restart` | 受理（現時点は no-op） |
| Flow | `supervision_stop/resume/restart` | 受理（現時点は no-op） |
| Sink | `supervision_stop/resume/restart` | 受理（現時点は no-op） |

## Restart with backoff（現時点）
| 対象 | API | 現在の挙動 |
|------|-----|------------|
| Source | `restart_source_with_backoff` | 受理（現時点は no-op） |
| Flow | `restart_flow_with_backoff` | 受理（現時点は no-op） |
| Sink | `restart_sink_with_backoff` | 受理（現時点は no-op） |

## recover 系
1. `recover(fallback)`  
   - 入力型: `Result<T, StreamError>`  
   - `Ok(T)` はそのまま通過  
   - `Err(_)` は `fallback` に置換
2. `recover_with_retries(max_retries, fallback)`  
   - 入力型: `Result<T, StreamError>`  
   - `Err(_)` 発生時、残リトライ回数があれば `fallback` を出力  
   - リトライ残数が 0 の場合は `StreamError::Failed` で失敗

## 今後の拡張余地
1. supervision 方針を実行器へ伝播し、演算子単位で `Stop/Resume/Restart` を差分適用する。
2. backoff を実時間（または tick）で実施し、再起動試行をスケジューリングする。
