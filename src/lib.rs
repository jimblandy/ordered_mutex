/*!

This crate provides wrappers around ordinary `Mutex` and `RwLock`
types that prevent deadlocks, by checking at runtime that locks are
acquired in a predetermined order.

To use this crate:

1)  Choose a ranking in which the locks in your code must be acquired: a
    thread may only acquire a lock whose rank is higher than any other lock
    it is already holding. The rank type must implement `Into<u32>`, and zero
    must represent the lowest rank: no locks held.


2)  Declare a thread-local variable to track which locks each thread is
    holding, and use it to implement the `ordered_mutex::Rank` trait
    for your order type:

2)  Use this crate's lock types in your program, specifying each lock's
    rank in its type:


 */

use std::marker::PhantomData;
use std::cell::Cell;

pub struct ThreadState<R> {
    current_rank: Cell<u32>,
    _rank_type: PhantomData<R>,
}

impl<R: Rank> ThreadState<R> {
    pub const fn new() -> Self {
        ThreadState {
            current_rank: Cell::new(0),
            _rank_type: PhantomData,
        }
    }
}

impl<R: Rank> ThreadState<R> {
    fn enter(&self, new: u32) -> SavedState<R> {
        let prior_rank = self.current_rank.replace(new);
        assert!(prior_rank < new);
        SavedState {
            prior_rank,
            _rank_type: PhantomData,
        }
    }
}

struct SavedState<R: Rank> {
    prior_rank: u32,
    _rank_type: PhantomData<R>,
}

impl<R: Rank> Drop for SavedState<R> {
    fn drop(&mut self) {
        R::CURRENT_RANK.with(|state| {
            state.current_rank.set(self.prior_rank);
        });
    }
}

pub trait Rank: Into<u32> + PartialOrd + 'static {
    const CURRENT_RANK: &'static std::thread::LocalKey<ThreadState<Self>>;
}

pub struct Mutex<T, R: Rank, const RANK: u32> {
    inner: std::sync::Mutex<T>,
    _rank_type: PhantomData<R>,
}

pub struct MutexGuard<'a, T: 'a, R: Rank> {
    inner: std::sync::MutexGuard<'a, T>,
    
    #[allow(dead_code)] // held for its `Drop`
    saved_state: SavedState<R>,
}

impl<T, R: Rank, const RANK: u32> Mutex<T, R, RANK> {
    pub fn new(value: T) -> Self {
        Mutex {
            inner: std::sync::Mutex::new(value),
            _rank_type: PhantomData,
        }
    }

    pub fn lock(&self) -> std::sync::LockResult<MutexGuard<T, R>> {
        let saved_state = R::CURRENT_RANK.with(|state| state.enter(RANK));
        match self.inner.lock() {
            Ok(inner) => Ok(MutexGuard {
                inner,
                saved_state,
            }),
            Err(inner_poison_error) => {
                Err(std::sync::PoisonError::new(MutexGuard {
                    inner: inner_poison_error.into_inner(),
                    saved_state,
                }))
            }
        }
    }
}

impl<'a, T, R: Rank> std::ops::Deref for MutexGuard<'a, T, R> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.inner.deref()
    }
}

impl<'a, T, R: Rank> std::ops::DerefMut for MutexGuard<'a, T, R> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.inner.deref_mut ()
    }
}
