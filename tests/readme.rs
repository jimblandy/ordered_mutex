ordered_mutex::define_rank! {
    /// Thread-local variable holding each thread's current GPU lock rank.
    static GPU_RANK;

    /// Order in which GPU locks must be acquired.
    #[repr(u32)]
    #[derive(Clone, PartialOrd, PartialEq)]
    enum GPULockRank {
        DeviceTracker,
        BufferMapState,
    }
}

struct Tracker;
struct BufferMapState;

use ordered_mutex::Mutex;

struct Device {
    tracker: Mutex<Tracker, GPULockRank>,
    // ...
}

struct Buffer {
    map_state: Mutex<BufferMapState, GPULockRank>,
    // ...
}

#[test]
fn in_order() {
    let device = Device {
        tracker: Mutex::new(Tracker, GPULockRank::DeviceTracker),
    };
    let buffer = Buffer {
        map_state: Mutex::new(BufferMapState, GPULockRank::BufferMapState),
    };

    {
        let _tracker_guard = device.tracker.lock().unwrap();
        let _map_state_guard = buffer.map_state.lock().unwrap();
    }

    {
        let _map_state_guard = buffer.map_state.lock().unwrap();
    }

    {
        let _tracker_guard = device.tracker.lock().unwrap();
    }

    {
        let _tracker_guard = device.tracker.lock().unwrap();
        let _map_state_guard = buffer.map_state.lock().unwrap();
    }
}

#[test]
#[should_panic]
fn out_of_order() {
    let device = Device {
        tracker: Mutex::new(Tracker, GPULockRank::DeviceTracker),
    };
    let buffer = Buffer {
        map_state: Mutex::new(BufferMapState, GPULockRank::BufferMapState),
    };

    let _map_state_guard = buffer.map_state.lock().unwrap();
    let _tracker_guard = device.tracker.lock().unwrap();
}

// Dropping lock guards out of order should still clear the state.
#[test]
fn staggered_clear() {
    let tracker: Mutex<(), GPULockRank> = Mutex::new((), GPULockRank::DeviceTracker);
    let map_state: Mutex<(), GPULockRank> = Mutex::new((), GPULockRank::BufferMapState);

    let tracker_guard = tracker.lock().unwrap();
    let map_state_guard = map_state.lock().unwrap();

    // Dropping the higher-ranked guard should return the thread to a
    // holding-no-locks state.
    drop(tracker_guard);
    drop(map_state_guard);

    let _second_tracker_guard = tracker.lock().unwrap();
}

// Dropping lock guards out of order should retain other guards.
#[test]
#[should_panic]
fn staggered_retain() {
    let tracker: Mutex<(), GPULockRank> = Mutex::new((), GPULockRank::DeviceTracker);
    let map_state: Mutex<(), GPULockRank> = Mutex::new((), GPULockRank::BufferMapState);

    let tracker_guard = tracker.lock().unwrap();
    let _map_state_guard = map_state.lock().unwrap();

    // Dropping the lower-ranked guard should not remove the
    // higher-ranked guard.
    drop(tracker_guard);
    let _second_tracker_guard = tracker.lock().unwrap();
}
