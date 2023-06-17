ordered_mutex::define_rank! {
    /// Thread-local variable holding each thread's current GPU lock rank.
    static GPU_RANK;

    /// Order in which GPU locks must be acquired.
    #[derive(Clone, Default, PartialOrd, PartialEq)]
    enum GPULockRank {
        Nothing,
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
