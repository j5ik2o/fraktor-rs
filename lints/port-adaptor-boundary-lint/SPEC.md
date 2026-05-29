# port-adaptor-boundary-lint

## ルール

`modules/*-adaptor-std/src` の public struct は、以下をしてはいけません。

- `ClusterApi` のような core concrete API facade を保持する。
- `CoreClusterApi` のような alias 経由で、core 型を同じ public name の wrapper として保持する。

adapter は引き続き core-defined port を実装できます。また、signature では core の value type、error、config type、port trait type を利用できます。
