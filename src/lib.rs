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

use std::cell::Cell;

pub trait Rank: Clone + Default + PartialOrd + Sized + 'static {
    const CURRENT_RANK: &'static std::thread::LocalKey<ThreadState<Self>>;
}

pub struct ThreadState<R> {
    current_rank: Cell<R>,
}

impl<R: Rank> ThreadState<R> {
    pub const fn new(init: R) -> Self {
        ThreadState {
            current_rank: Cell::new(init),
        }
    }
}

impl<R: Rank> ThreadState<R> {
    fn enter(new_rank: R) -> SavedState<R> {
        let prior_rank = R::CURRENT_RANK.with(|state| state.current_rank.replace(new_rank.clone()));
        assert!(prior_rank < new_rank);
        SavedState { prior_rank }
    }

    fn exit(prior_rank: R) {
        R::CURRENT_RANK.with(|state| {
            state.current_rank.set(prior_rank);
        });
    }
}

struct SavedState<R: Rank> {
    prior_rank: R,
}

impl<R: Rank> Drop for SavedState<R> {
    fn drop(&mut self) {
        ThreadState::exit(self.prior_rank.clone());
    }
}

pub struct Mutex<T, R: Rank> {
    inner: std::sync::Mutex<T>,
    rank: R,
}

pub struct MutexGuard<'a, T: 'a, R: Rank> {
    inner: std::sync::MutexGuard<'a, T>,

    #[allow(dead_code)] // held for its `Drop`
    saved_state: SavedState<R>,
}

impl<T, R: Rank> Mutex<T, R> {
    pub fn new(value: T, rank: R) -> Self {
        Mutex {
            inner: std::sync::Mutex::new(value),
            rank,
        }
    }

    pub fn lock(&self) -> std::sync::LockResult<MutexGuard<T, R>> {
        let saved_state = ThreadState::enter(self.rank.clone());
        match self.inner.lock() {
            Ok(inner) => Ok(MutexGuard { inner, saved_state }),
            Err(inner_poison_error) => Err(std::sync::PoisonError::new(MutexGuard {
                inner: inner_poison_error.into_inner(),
                saved_state,
            })),
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
        self.inner.deref_mut()
    }
}

#[macro_export]
macro_rules! define_rank {
    {
        $( #[ $( $current_attr:meta ),* ] )*
        static $current_rank:ident;
    
        $( #[ $( $type_attr:meta ),* ] )*
        enum $rank_type:ident {
            $( $variant:ident, )*
        }
    } => {
        $( #[ $( $type_attr ),* ] )*
        enum $rank_type {
            #[default]
            $( $variant ),*
        }

        thread_local! {
            $( #[ $( $current_attr ),* ] )*
            static $current_rank: $crate::ThreadState<$rank_type> = $crate::ThreadState::new($rank_type::default());
        }

        impl $crate::Rank for $rank_type {
            const CURRENT_RANK: &'static std::thread::LocalKey<$crate::ThreadState<Self>> = &$current_rank;
        }
    }
}
