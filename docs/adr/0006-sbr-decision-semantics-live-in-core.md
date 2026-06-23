# SBR decision semantics は core に置く

Split Brain Resolution の decision semantics は `cluster-core` / downing-provider contract に置き、std/provider code は lifecycle binding と具体的な lease backend integration を所有する。core は port-style vocabulary 経由で lease acquisition outcome を受け取ることで、`no_std` 下でも downing policy を testable に保ち、provider ごとに strategy semantics を再実装して分岐することを防ぐ。
