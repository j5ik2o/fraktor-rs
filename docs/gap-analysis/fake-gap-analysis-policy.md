# fake gap 分析ポリシー

## 目的

この文書は、Pekko 互換分析における `fake gap` の定義と判断基準を共通化する。

通常の gap analysis が「未実装機能」を扱うのに対し、fake gap analysis は
**「見た目は埋まっているが、実質は別物」** な箇所を対象にする。

## fake gap の定義

以下のいずれかに該当する場合を fake gap とみなす。

- Pekko と同名または近い API があるが、返す情報量や契約が薄い
- 公開 API はあるが、内部責務の切り方が Pekko と大きく異なり、将来 parity 実装の足場になりにくい
- 互換 API が存在していても、内部 state machine や意味論が Pekko より縮退している
- API 名はあるが、実際には no-op / placeholder / fallback に留まる

## 判断原則

Pekko parity を見る際も、優先順位は次の通りとする。

1. Rust / fraktor-rs の設計原則
2. 利用者が観測する振る舞い契約
3. API surface の近さ
4. 内部構造の近さ

つまり、Pekko に似ていても Rust 的に不自然な設計は採用しない。

## 改善方針

fake gap に対しては、次の方針を優先する。

- 互換ラッパーを足してごまかさない
- fallback 実装で逃がさない
- 実装できていない互換 API は一度 public surface から外す
- 必要なら破壊的変更を許容して、公開契約と内部実装の境界を作り直す

## 読み方

各 `${module}-fake-gap-analysis.md` には、次だけを書く。

- そのモジュール固有の fake gap
- そのモジュール固有の改善策
- そのモジュール固有の結論
