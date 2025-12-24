use super::*;

impl<TB: RuntimeToolbox> Scheduler<TB> {
  /// Returns the number of registered jobs (testing helper).
  pub fn job_count_for_test(&self) -> usize {
    self.jobs.len()
  }

  /// Returns the command associated with the provided handle for testing.
  pub fn command_for_test(&self, handle: &SchedulerHandle) -> Option<&SchedulerCommand<TB>> {
    self.jobs.get(&handle.raw()).map(|job| &job.command)
  }

  /// Advances the scheduler by the requested ticks (testing helper).
  pub fn run_for_test(&mut self, ticks: u64) {
    self.run_for_ticks(ticks);
  }
}
