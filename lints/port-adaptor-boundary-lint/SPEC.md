# port-adaptor-boundary-lint

## Rule

In `modules/*-adaptor-std/src`, public structs must not:

- store core concrete API facades such as `ClusterApi`;
- wrap a core type with the same public name via aliases such as `CoreClusterApi`.

Adapters may still implement core-defined ports and may use core value types, errors, config types, and port trait types in signatures.
