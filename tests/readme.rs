ordered_mutex::define_rank! {
    /// Thread-local variable holding each thread's current GPU lock rank.
    static GPU_RANK;

    /// Order in which GPU locks must be acquired.
    #[derive(Clone, Default, PartialOrd, PartialEq)]
    enum GPULockOrder {
        Nothing,
        DeviceTracker,
        BufferMapState,
    }
}

struct Tracker;
struct BufferMapState;

use ordered_mutex::Mutex;

struct Device {
    tracker: Mutex<Tracker, GPULockOrder>,
    // ...
}

struct Buffer {
    map_state: Mutex<BufferMapState, GPULockOrder>,
    // ...
}

#[test]
fn in_order() {
    let device = Device {
        tracker: Mutex::new(Tracker, GPULockOrder::DeviceTracker),
    };
    let buffer = Buffer {
        map_state: Mutex::new(BufferMapState, GPULockOrder::BufferMapState),
    };

    {
        let _tracker_guard = device.tracker.lock();
        let _map_state_guard = buffer.map_state.lock();
    }

    {
        let _map_state_guard = buffer.map_state.lock();
    }

    {
        let _tracker_guard = device.tracker.lock();
    }

    {
        let _tracker_guard = device.tracker.lock();
        let _map_state_guard = buffer.map_state.lock();
    }
}

#[test]
#[should_panic]
fn out_of_order() {
    let device = Device {
        tracker: Mutex::new(Tracker, GPULockOrder::DeviceTracker),
    };
    let buffer = Buffer {
        map_state: Mutex::new(BufferMapState, GPULockOrder::BufferMapState),
    };

    let _map_state_guard = buffer.map_state.lock();
    let _tracker_guard = device.tracker.lock();
}
