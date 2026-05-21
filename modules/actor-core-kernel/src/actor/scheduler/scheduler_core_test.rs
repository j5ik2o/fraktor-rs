use super::*;

impl Scheduler {
  /// Returns the number of registered jobs (testing helper).
  #[must_use]
  pub fn job_count_for_test(&self) -> usize {
    self.jobs.len()
  }

  /// Returns the command associated with the provided handle for testing.
  #[must_use]
  pub fn command_for_test(&self, handle: &SchedulerHandle) -> Option<&SchedulerCommand> {
    self.jobs.get(&handle.raw()).map(|job| &job.command)
  }

  /// Removes the job entry while leaving the cancellable registry intact.
  pub fn remove_job_for_test(&mut self, handle: &SchedulerHandle) -> bool {
    self.jobs.remove(&handle.raw()).is_some()
  }

  /// Advances the scheduler by the requested ticks (testing helper).
  pub fn run_for_test(&mut self, ticks: u64) {
    self.run_for_ticks(ticks);
  }
}
