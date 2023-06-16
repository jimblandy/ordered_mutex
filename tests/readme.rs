#[repr(u32)]
#[derive(PartialEq, PartialOrd)]
enum GPULockOrder {
    Nothing,
    DeviceTracker,
    BufferMapState,
    CommandEncoder
}

impl Into<u32> for GPULockOrder {
    fn into(self) -> u32 { self as _ }
}

use ordered_mutex::{Rank, ThreadState};

thread_local! {
    static GPU_RANK: ThreadState<GPULockOrder> = ThreadState::new();
}

impl Rank for GPULockOrder {
    const CURRENT_RANK: &'static std::thread::LocalKey<ThreadState<Self>> = &GPU_RANK;
}

struct Tracker;
struct BufferMapState;

use ordered_mutex::Mutex;    

struct Device {
    tracker: Mutex<Tracker, GPULockOrder, { GPULockOrder::DeviceTracker as u32 }>,
    // ...
}

struct Buffer {
    map_state: Mutex<BufferMapState, GPULockOrder, { GPULockOrder::BufferMapState as u32 }>,
    // ...
}

#[test]
fn in_order() {
    let device = Device { tracker: Mutex::new(Tracker) };
    let buffer = Buffer { map_state: Mutex::new(BufferMapState) };

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
    let device = Device { tracker: Mutex::new(Tracker) };
    let buffer = Buffer { map_state: Mutex::new(BufferMapState) };

    let _map_state_guard = buffer.map_state.lock();
    let _tracker_guard = device.tracker.lock();
}
