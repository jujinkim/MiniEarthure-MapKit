//! Job-local cooperative cancellation. Tokens never reset or cancel another job.
use std::{cell::RefCell, sync::{Arc, Mutex, atomic::{AtomicBool, AtomicU64, Ordering}}};
/// A stage-local count, never an estimate of whole-job completion. None means
/// the work size is unknown (notably candidate search).
#[derive(Clone, Debug, serde::Serialize, PartialEq, Eq)]
pub struct Progress {
    pub job_id: u64,
    pub revision: u64,
    pub stage: String,
    pub completed: u64,
    pub total: Option<u64>,
    pub unit: String,
}
#[derive(Debug)]
struct State { cancelled: AtomicBool, progress: Mutex<Progress> }
#[derive(Clone, Debug)]
pub struct CancellationToken(Arc<State>);
impl Default for CancellationToken {
    fn default() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        Self(Arc::new(State { cancelled: AtomicBool::new(false), progress: Mutex::new(Progress {
            job_id: NEXT.fetch_add(1, Ordering::Relaxed), revision: 0,
            stage: "preparing".into(), completed: 0, total: None, unit: "items".into(),
        }) }))
    }
}
thread_local! { static CURRENT: RefCell<Option<CancellationToken>> = const { RefCell::new(None) }; }
impl CancellationToken {
    pub fn cancel(&self) { self.0.cancelled.store(true, Ordering::Release); }
    pub fn is_cancelled(&self) -> bool { self.0.cancelled.load(Ordering::Acquire) }
    pub fn progress(&self) -> Progress { self.0.progress.lock().unwrap().clone() }
    pub fn report(&self, stage: &str, completed: u64, total: Option<u64>, unit: &str) {
        if self.is_cancelled() { return; }
        let mut p = self.0.progress.lock().unwrap();
        let total = total.filter(|v| *v > 0);
        let completed = total.map_or(completed, |n| completed.min(n));
        if p.stage == stage && p.completed == completed && p.total == total && p.unit == unit { return; }
        p.revision += 1;
        p.stage = stage.into(); p.completed = completed; p.total = total; p.unit = unit.into();
    }
    /// Worker entry/exit only. Nested native callers inherit the same job token.
    pub fn enter(&self) { CURRENT.with(|v| *v.borrow_mut() = Some(self.clone())); }
    pub fn leave() { CURRENT.with(|v| *v.borrow_mut() = None); }
    pub fn run<T>(&self, work: impl FnOnce() -> crate::Result<T>) -> crate::Result<T> {
        struct Reset(Option<CancellationToken>);
        impl Drop for Reset { fn drop(&mut self) { CURRENT.with(|v| *v.borrow_mut() = self.0.take()); } }
        let _reset = Reset(CURRENT.with(|v| v.replace(Some(self.clone()))));
        checkpoint()?;
        work()
    }
}
pub fn progress(stage: &str, completed: u64, total: Option<u64>, unit: &str) {
    CURRENT.with(|v| { if let Some(token) = v.borrow().as_ref() { token.report(stage, completed, total, unit); } });
}
pub fn checkpoint() -> crate::Result<()> {
    if CURRENT.with(|v| v.borrow().as_ref().is_some_and(CancellationToken::is_cancelled)) {
        Err(crate::error("E_CANCELLED", "generation job cancelled"))
    } else { Ok(()) }
}
#[cfg(test)] mod tests {
 use super::*;
 #[test] fn running_job_observes_cancel_without_cancelling_replacement() {
  let token=CancellationToken::default(); let worker_token=token.clone();
  let (entered,ready)=std::sync::mpsc::channel(); let (resume,wait)=std::sync::mpsc::channel();
  let worker=std::thread::spawn(move || worker_token.run(|| {
   entered.send(()).unwrap(); wait.recv().unwrap(); checkpoint()
  }));
  ready.recv().unwrap(); token.cancel(); resume.send(()).unwrap();
  assert_eq!(worker.join().unwrap().unwrap_err().code,"E_CANCELLED");
  CancellationToken::default().run(|| checkpoint()).unwrap();
 }
 #[test] fn jobs_are_independent_and_scopes_restore() {
  let old=CancellationToken::default(); old.cancel();
  assert!(old.run(|| Ok(())).is_err()); assert!(checkpoint().is_ok());
  let fresh=CancellationToken::default();
  fresh.run(|| { assert!(old.run(|| Ok(())).is_err()); checkpoint() }).unwrap();
 }
 #[test] fn progress_is_atomic_stage_local_and_job_scoped() {
  let token = CancellationToken::default(); let fresh = CancellationToken::default();
  assert_ne!(token.progress().job_id, fresh.progress().job_id);
  token.run(|| {
   progress("searching", 17, None, "candidates");
   assert_eq!(token.progress().total, None);
   fresh.run(|| { progress("saving", 12, Some(20), "bytes"); Ok(()) })?;
   progress("validating", 1, Some(4), "checks"); Ok(())
  }).unwrap();
  let worker = token.clone();
  std::thread::spawn(move || worker.report("saving", 99, Some(100), "bytes")).join().unwrap();
  let p = token.progress(); assert_eq!((p.completed, p.total, p.unit.as_str()), (99, Some(100), "bytes"));
  assert_eq!(fresh.progress().completed, 12);
  token.cancel(); token.report("ready", 1, Some(1), "items"); assert_eq!(token.progress(), p);
 }
}
