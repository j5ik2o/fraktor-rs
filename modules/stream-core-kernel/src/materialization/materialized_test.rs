use super::Materialized;
use crate::r#impl::materialization::StreamShared;

impl<Mat> Materialized<Mat> {
  pub(crate) const fn stream(&self) -> &StreamShared {
    &self.stream
  }
}
