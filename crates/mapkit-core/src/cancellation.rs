//! Job-local cooperative cancellation. Tokens never reset or cancel another job.
use std::{cell::RefCell, sync::{Arc, atomic::{AtomicBool, Ordering}}};
#[derive(Clone, Debug, Default)]
pub struct CancellationToken(Arc<AtomicBool>);
thread_local! { static CURRENT: RefCell<Option<CancellationToken>> = const { RefCell::new(None) }; }
impl CancellationToken {
    pub fn cancel(&self) { self.0.store(true, Ordering::Release); }
    pub fn is_cancelled(&self) -> bool { self.0.load(Ordering::Acquire) }
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
}
