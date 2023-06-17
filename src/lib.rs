//! This crate provides wrappers around ordinary `Mutex` and `RwLock`
//! types that prevent deadlocks, by checking at runtime that locks are
//! acquired in a predetermined order.
//!
//! # Deadlock prevention
//!
//! When a program deadlocks waiting to acquire a lock, it is often
//! the case that two threads are each holding a lock the other thread
//! is trying to acquire. There's nothing wrong with a thread holding
//! one lock while trying to acquire another, but if two threads ever
//! happen to fall into this pattern, then neither of them can make
//! any further progress, and a deadlock has occurred.
//!
//! More generally, in any deadlock involving only waiting to acquire
//! locks, there must be a directed cycle of threads, each of which is
//! holding a lock that the next thread in the cycle is waiting for.
//! Thus, one simple and sufficient way to prevent deadlocks is to
//! impose a partial order, or "ranking", on all the program's locks,
//! and forbid threads from acquiring any lock unless it outranks the
//! locks it already holds. This prevents any such cycles from
//! forming.
//!
//! This crate provides wrappers for `Mutex` and `RwLock` that track
//! the highest rank of lock that each thread currently holds, and
//! panic if a thread violates the order. You specify the ranking, in
//! the form of an enum that implements [`PartialOrd`], [`Clone`], and
//! [`Into<u32>`]. You indicate the rank of each lock when you create
//! it.
//!
//! Note that this analysis is strictly thread-local, evaluating each
//! thread's behavior in isolation. It does not depend on any deadlock
//! actually occurring to report a particular thread's misbehavior.
//! This makes problems easier to reproduce, since it is independent
//! of how threads' execution interleaves.
//!
//! # How to use this crate
//!
//! 1)  Choose a ranking in which the locks in your code must be acquired: a
//!     thread may only acquire a lock whose rank is higher than any other lock
//!     it is already holding. Use this crate's `define_rank!` macro to
//!     define an `enum` representing that ranking:
//!
//!         ordered_mutex::define_rank! {
//!             /// Thread-local variable holding each thread's current GPU lock rank.
//!             static GPU_RANK;
//!
//!             /// Order in which GPU locks must be acquired.
//!             #[repr(u32)]
//!             #[derive(Clone, PartialOrd, PartialEq)]
//!             enum GPULockRank {
//!                 DeviceTracker,
//!                 BufferMapState,
//!             }
//!         }
//!
//!     This defines the `GPULockRank` enum, declares a thread-local
//!     variable named `GPU_RANK`, and implements this crate's
//!     [`Rank`] trait for `GPULockRank`.
//!
//!     Note that the rank enum must implement the standard library's
//!     [`Clone`] and [`PartialOrd`] traits.
//!
//!     Further, to simplify implementation, the rank enum must
//!     implement `Into<u32>`, and variants must have values less than
//!     64. The `define_rank!` macro requires that the enum be
//!     convertable to `u32` via the `as` operator, and generates an
//!     implementation of `From<u32>` for it automatically; this
//!     effectively requires the enum to use `#[repr(u32)]`, as shown
//!     in the example.
//!
//! 2)  Use this crate's [`Mutex`] and [`RwLock`] types to protect your data structures,
//!     supplying your rank type as a second generic parameter:
//!
//!         # ordered_mutex::define_rank! {
//!         #     static GPU_RANK;
//!         #     #[derive(Clone, PartialOrd, PartialEq)]
//!         #     enum GPULockRank { Nothing, DeviceTracker, BufferMapState, }
//!         # }
//!         # struct Tracker;
//!         # struct BufferMapState;
//!         use ordered_mutex::Mutex;
//!         
//!         struct Device {
//!             tracker: Mutex<Tracker, GPULockRank>,
//!             // ...
//!         }
//!         
//!         struct Buffer {
//!             map_state: Mutex<BufferMapState, GPULockRank>,
//!             // ...
//!         }
//!
//! 3)  Supply each lock's rank when you create it:
//!
//!         # ordered_mutex::define_rank! {
//!         #     static GPU_RANK;
//!         #     #[derive(Clone, PartialOrd, PartialEq)]
//!         #     enum GPULockRank { Nothing, DeviceTracker, BufferMapState, }
//!         # }
//!         # use ordered_mutex::Mutex;
//!         # struct Tracker;
//!         # struct BufferMapState;
//!         # struct Device { tracker: Mutex<Tracker, GPULockRank>, }
//!         # struct Buffer { map_state: Mutex<BufferMapState, GPULockRank>, }
//!         let device = Device {
//!             tracker: Mutex::new(Tracker, GPULockRank::DeviceTracker),
//!             // ...
//!         };
//!
//!         let buffer = Buffer {
//!             map_state: Mutex::new(BufferMapState, GPULockRank::BufferMapState),
//!             // ...
//!         };
//!
//! 4)  Acquire and release locks as usual. If any thread ever tries to
//!     acquire a lower-ranked lock while holding a higher-ranked
//!     lock, the lock operation will panic.
//!
//! # Parking lot
//!
//! At the moment, this crate simply wraps the [`parking_lot`] crate's
//! locks, but there's nothing about this instrumentation that is
//! specific to `parking_lot`. In the future, this crate should
//! provide generic types that can wrap any lock that provides the
//! necessary interfaces. And it should support both `parking_lot` and
//! the Rust standard library's locks out of the box.
//!
//! # Why not atomics?
//!
//! Although they're not implemented this way, an atomic type like
//! [`std::sync::atomic::AtomicU32`] behaves like a [`Mutex`] wrapped
//! around some simple value type. This crate, however, only defines
//! wrappers for lock types, and doesn't deal with atomics at all.
//! That's because the kind of deadlock described here can only occur
//! when a thread holds one lock while trying to acquire another.
//! Atomic types provide only a fixed set of operations, none of which
//! ever try to acquire some other lock, so atomics cannot participate
//! in deadlocks.
//!
//! In general, any lock that is never held while trying to acquire
//! another lock cannot participate in a deadlock. This is the
//! category that atomics fall into.
//!
//! # Lock ranks are not very modular
//!
//! It can be tricky to establish the boundaries of the code that must
//! have its locks included in a particular ranking. Deadlocks are
//! built by threads holding one lock while acquiring another---but
//! that second acquisition might take place in a callee of a callee
//! of a callee of the function that acquired the first lock, in an
//! entirely different crate.
//!
//! Imagine a global graph of all the locks in the entire program
//! (dependent crates and the standard library included) where an edge
//! from one lock to another indicates that a thread might acquire the
//! second while holding the first. This graph must have no cycles.
//!
//! It's possible in some cases to be sure a lock is irrelevant. For
//! example, if some lock is used only internally to a crate, and is
//! never held while any other lock is acquired, it obviously can't
//! participate in any cycle, so it can be ignored. If all a crate's
//! locks fall into this category, then clearly any call to that crate
//! is benign.
//!
//! Interfaces that use callback functions can make this sort of
//! analysis very difficult. In general, one would want to assume that
//! a callback might do anything at all, so the set of locks it might
//! try to acquire is unknown.
//!
//! Rust's locks are flexible in various ways that also make analysis
//! tricky:
//!
//! - Rust permits a function that acquires a lock to return the
//!   guard, so it's not technically correct to assume that locks are
//!   scoped like the function call graph.
//!
//! - Similarly, Rust permits lock guards to be dropped in any order,
//!   so it's not correct to assume that locking activity nests
//!   nicely.
//!
//! # Const generics might be nice
//!
//! It would make sense for a given lock's rank to be built into its
//! type, rather than passing it as a parameter when it was created.
//! This would ensure that ranks were written out in data type
//! definitions, which is good documentation, and prevent exchanging
//! locks of different ranks.
//!
//! One way to accomplish this would be to have the rank be a const
//! generic parameter of the lock wrapper type. However, Rust only
//! permits const generic parameters to have primitive types, and
//! using numbers for ranks seems bad. The unstable
//! `"adt_const_params"` feature would relax this restriction, but it
//! doesn't seem to be a priority.

use std::cell::RefCell;

mod rank_set;

use rank_set::RankSet;

pub trait Rank: PartialOrd + Into<u32> + Clone + Sized + 'static {
    const CURRENT_RANK: &'static std::thread::LocalKey<ThreadState<Self>>;
}

pub struct ThreadState<R> {
    current_rank: RefCell<RankSet<R>>,
}

impl<R: Rank> ThreadState<R> {
    pub const fn new() -> Self {
        ThreadState {
            current_rank: RefCell::new(RankSet::new()),
        }
    }
}

impl<R: Rank> ThreadState<R> {
    fn lock(rank: R) -> SavedState<R> {
        R::CURRENT_RANK.with(|state| {
            assert!(
                !state.current_rank.borrow_mut().insert(rank.clone()),
                "Attempted to acquire lock out of order"
            );
        });
        SavedState { rank }
    }

    fn unlock(rank: R) {
        R::CURRENT_RANK.with(|state| {
            state.current_rank.borrow_mut().remove(rank);
        });
    }
}

struct SavedState<R: Rank> {
    rank: R,
}

impl<R: Rank> Drop for SavedState<R> {
    fn drop(&mut self) {
        ThreadState::unlock(self.rank.clone());
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
        let saved_state = ThreadState::lock(self.rank.clone());
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
            $( $variant ),*
        }

        thread_local! {
            $( #[ $( $current_attr ),* ] )*
            static $current_rank: $crate::ThreadState<$rank_type> = $crate::ThreadState::new();
        }

        impl $crate::Rank for $rank_type {
            const CURRENT_RANK: &'static std::thread::LocalKey<$crate::ThreadState<Self>> = &$current_rank;
        }

        impl From<$rank_type> for u32 {
            fn from(value: $rank_type) -> u32 { value as _ }
        }
    }
}
