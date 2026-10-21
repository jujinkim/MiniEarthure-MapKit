//! Budget profiles. `Runtime` (the default, and every reader of received
//! packages) refuses each budget exactly as before. `Authoring` is entered only
//! by Editor entry points on the user's own data: a value above a Runtime budget
//! becomes a [`Warning`], and only a raised anti-hang/anti-OOM guard still fails
//! with the original error. Work counters never change outputs, only pass/fail.
//! v1 format invariants never consult the profile.
use serde::Serialize;
use std::{borrow::Cow, cell::RefCell};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Profile {
    #[default]
    Runtime,
    Authoring,
}

/// One entry per code; `actual` is the largest value seen in the scope.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Warning {
    pub code: Cow<'static, str>,
    pub actual: u64,
    pub limit: u64,
    pub message: Cow<'static, str>,
    /// The game (Runtime profile) refuses a map or package with this value.
    pub runtime_rejects: bool,
}

/// A Runtime budget and its raised Authoring guard.
#[derive(Debug, Clone, Copy)]
pub struct Budget {
    pub code: &'static str,
    pub limit: u64,
    pub guard: u64,
    pub message: &'static str,
    pub runtime_rejects: bool,
}
impl Budget {
    /// Inclusive limit for the active profile (allocation and scan bounds).
    pub fn active_limit(&self) -> u64 {
        if Profile::current() == Profile::Authoring { self.guard } else { self.limit }
    }
    /// Runtime: `actual > limit` fails with `err`. Authoring: `actual > guard`
    /// fails with `err`, `actual > limit` records a warning.
    pub fn check(&self, actual: u64, err: impl FnOnce() -> crate::Error) -> crate::Result<()> {
        if actual <= self.limit {
            return Ok(());
        }
        // One thread-local access per call; per-tick callers only raise `actual`.
        let warned = actual <= self.guard && SCOPE.with(|s| {
            let mut scope = s.borrow_mut();
            let Some(list) = scope.as_mut() else { return false };
            match list.iter_mut().find(|w| w.code == self.code) {
                Some(known) => {
                    known.actual = known.actual.max(actual);
                    known.runtime_rejects |= self.runtime_rejects;
                }
                None => list.push(Warning {
                    code: Cow::Borrowed(self.code),
                    actual,
                    limit: self.limit,
                    message: Cow::Borrowed(self.message),
                    runtime_rejects: self.runtime_rejects,
                }),
            }
            true
        });
        if warned { Ok(()) } else { Err(err()) }
    }
}

thread_local! { static SCOPE: RefCell<Option<Vec<Warning>>> = const { RefCell::new(None) }; }

impl Profile {
    pub fn current() -> Profile {
        SCOPE.with(|s| if s.borrow().is_some() { Profile::Authoring } else { Profile::Runtime })
    }
    /// Run `work` under this profile on the current thread. Runtime runs it
    /// unchanged with no warnings. A nested Authoring scope also reports its
    /// warnings to the enclosing scope.
    pub fn run<T>(self, work: impl FnOnce() -> crate::Result<T>) -> (crate::Result<T>, Vec<Warning>) {
        if self == Profile::Runtime {
            struct Restore(Option<Vec<Warning>>);
            impl Drop for Restore {
                fn drop(&mut self) { let outer = self.0.take(); SCOPE.with(|s| *s.borrow_mut() = outer); }
            }
            let _restore = Restore(SCOPE.with(|s| s.borrow_mut().take()));
            return (work(), vec![]);
        }
        struct Restore(Option<Option<Vec<Warning>>>);
        impl Drop for Restore {
            fn drop(&mut self) {
                if let Some(outer) = self.0.take() { SCOPE.with(|s| *s.borrow_mut() = outer); }
            }
        }
        let mut restore = Restore(Some(SCOPE.with(|s| s.borrow_mut().replace(vec![])))) ;
        let result = work();
        let warnings = SCOPE.with(|s| s.borrow_mut().take()).unwrap_or_default();
        let outer = restore.0.take().unwrap();
        SCOPE.with(|s| *s.borrow_mut() = outer);
        for w in &warnings { note(w.clone()); }
        (result, warnings)
    }
}

/// Accept/reject test of a seeded or suggested candidate: always the Runtime
/// profile, so the Editor accepts exactly the candidates the game accepts.
/// Warnings noted inside never reach an enclosing Authoring scope.
pub fn runtime<T>(work: impl FnOnce() -> crate::Result<T>) -> crate::Result<T> {
    Profile::Runtime.run(work).0
}

/// Record a warning in the active Authoring scope; ignored under Runtime.
pub fn note(warning: Warning) {
    SCOPE.with(|s| {
        if let Some(list) = s.borrow_mut().as_mut() {
            if let Some(known) = list.iter_mut().find(|w| w.code == warning.code) {
                known.actual = known.actual.max(warning.actual);
                known.runtime_rejects |= warning.runtime_rejects;
            } else {
                list.push(warning);
            }
        }
    });
}

/// Generated-cell triangle cap used by every Runtime generation call.
pub const RUNTIME_TRIANGLES: usize = 500_000;
/// Editor generation cap; equals the internal estimate clamp.
pub const AUTHORING_TRIANGLES: usize = 2_000_000;
/// Warn when an Authoring generation produced more triangles than Runtime allows.
pub fn note_triangles(count: usize) {
    if count > RUNTIME_TRIANGLES {
        note(Warning {
            code: "cell_triangles".into(),
            actual: count as u64,
            limit: RUNTIME_TRIANGLES as u64,
            message: "Cell has more triangles than the game's per-cell limit".into(),
            runtime_rejects: true,
        });
    }
}
/// Editor-supplied generation cap: 1..=2,000,000 triangles.
pub fn authoring_triangles(max_triangles: i64) -> crate::Result<usize> {
    if !(1..=AUTHORING_TRIANGLES as i64).contains(&max_triangles) {
        return Err(crate::error("E_BUDGET", "triangle allowance must be 1..2000000"));
    }
    Ok(max_triangles as usize)
}

#[cfg(test)]
mod tests {
    use super::*;
    const B: Budget = Budget { code: "x", limit: 10, guard: 80, message: "X", runtime_rejects: true };
    #[test]
    fn runtime_refuses_authoring_warns_until_guard() {
        assert_eq!(B.check(11, || crate::error("E_LIMIT", "x")).unwrap_err().code, "E_LIMIT");
        let (r, w) = Profile::Authoring.run(|| { B.check(11, || crate::error("E_LIMIT", "x"))?; B.check(30, || crate::error("E_LIMIT", "x")) });
        r.unwrap();
        assert_eq!(w, vec![Warning { code: "x".into(), actual: 30, limit: 10, message: "X".into(), runtime_rejects: true }]);
        let (r, _) = Profile::Authoring.run(|| B.check(81, || crate::error("E_LIMIT", "x")));
        assert_eq!(r.unwrap_err().code, "E_LIMIT");
        assert_eq!(Profile::current(), Profile::Runtime);
    }
    #[test]
    fn candidate_tests_use_runtime_and_drop_their_warnings() {
        let (r, w) = Profile::Authoring.run(|| {
            assert_eq!(runtime(|| B.check(20, || crate::error("E_LIMIT", "x"))).unwrap_err().code, "E_LIMIT");
            assert_eq!(Profile::current(), Profile::Authoring);
            Ok(())
        });
        assert!(r.is_ok() && w.is_empty());
    }
    #[test]
    fn nested_scopes_report_outward_and_runtime_scope_suspends_authoring() {
        let (_, outer) = Profile::Authoring.run(|| {
            let (_, inner) = Profile::Authoring.run(|| B.check(20, || crate::error("E", "")));
            assert_eq!(inner.len(), 1);
            let (r, none) = Profile::Runtime.run(|| B.check(20, || crate::error("E", "")));
            assert!(r.is_err() && none.is_empty());
            assert_eq!(Profile::current(), Profile::Authoring);
            Ok(())
        });
        assert_eq!(outer.len(), 1);
    }
}
