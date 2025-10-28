// 各TypeKeyに対応する能力トレイト実装をまとめる。
use super::{MultiProducer, SingleConsumer, SingleProducer, SupportsPeek};
use crate::v2::collections::queue::{FifoKey, MpscKey, PriorityKey, SpscKey};

impl MultiProducer for MpscKey {}
impl SingleConsumer for MpscKey {}

impl SingleProducer for SpscKey {}
impl SingleConsumer for SpscKey {}

impl SingleProducer for FifoKey {}
impl SingleConsumer for FifoKey {}

impl SingleProducer for PriorityKey {}
impl SingleConsumer for PriorityKey {}
impl SupportsPeek for PriorityKey {}
