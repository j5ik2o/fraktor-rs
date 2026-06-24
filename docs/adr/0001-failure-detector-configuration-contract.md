# Failure Detector Configuration is an observation contract

`FailureDetectorConfig` は Failure Detector Algorithm Selection (故障検出器アルゴリズム選択) を公開するための型ではなく、Failure Detector (故障検出器) が Availability Evidence (可用性観測証拠) をどう観測するかを調整する Cluster Configuration (クラスタ設定) として扱う。現時点では Phi Accrual 前提の観測パラメータを持ち、Join Compatibility (参加互換性) には単一の `cluster.failure-detector` キーとして含め、詳細な差分は Compatibility Mismatch Reason (互換性不一致理由) で示す。
