use std::cell::{Cell, RefCell};
#[derive(Clone, Copy)]
pub(crate) struct Stamp {
    ticks: u64,
    cpu: u32,
}
impl Stamp {
    #[inline]
    pub(crate) fn read() -> Self {
        #[cfg(target_arch = "x86_64")]
        {
            // Measurement branches run on CPUs with RDTSCP. LFENCE prevents the measured
            // loads from crossing the timestamp boundary; AUX detects thread migration.
            let mut cpu = 0;
            let ticks = unsafe {
                core::arch::x86_64::_mm_lfence();
                let ticks = core::arch::x86_64::__rdtscp(&mut cpu);
                core::arch::x86_64::_mm_lfence();
                ticks
            };
            Self { ticks, cpu }
        }
        #[cfg(not(target_arch = "x86_64"))]
        {
            Self { ticks: 0, cpu: 0 }
        }
    }
    pub(crate) fn elapsed(self, end: Self) -> Option<u64> {
        (self.cpu == end.cpu).then(|| end.ticks.checked_sub(self.ticks)).flatten()
    }
}

#[derive(Default)]
struct Stats {
    count: u64,
    samples: u64,
    accumulate_ticks: u64,
    cache_ticks: u64,
    build_ticks: u64,
    hook_ticks: u64,
    dropped: u64,
}
thread_local! { static STATS: RefCell<Stats> = RefCell::new(Stats::default()); }
thread_local! { static SELECTED: Cell<bool> = const { Cell::new(false) }; }
/// Selects a nested commit measurement from the outer executor's randomized sample.
pub fn select(selected: bool) {
    SELECTED.with(|value| value.set(selected));
}
pub(crate) fn start() -> Option<Stamp> {
    STATS.with(|stats| stats.borrow_mut().count += 1);
    SELECTED.with(|value| value.get()).then(Stamp::read)
}
pub(crate) fn record(start: Option<Stamp>, built: Option<Stamp>, middle: Option<Stamp>) {
    if let (Some(start), Some(built), Some(middle)) = (start, built, middle) {
        let end = Stamp::read();
        STATS.with(|stats| {
            let mut stats = stats.borrow_mut();
            if let (Some(accumulate), Some(cache), Some(build), Some(hook)) = (
                start.elapsed(middle),
                middle.elapsed(end),
                start.elapsed(built),
                built.elapsed(middle),
            ) {
                stats.samples += 1;
                stats.accumulate_ticks += accumulate;
                stats.cache_ticks += cache;
                stats.build_ticks += build;
                stats.hook_ticks += hook;
            } else {
                stats.dropped += 1;
            }
        });
    }
}
pub(crate) fn emit() {
    STATS.with(|stats| {
        let stats = std::mem::take(&mut *stats.borrow_mut());
        tracing::info!(target: "tempo_phase_measure", thread_id = ?std::thread::current().id(),
            thread_name = std::thread::current().name().unwrap_or("unknown"), count = stats.count, samples = stats.samples,
            accumulate_ticks = stats.accumulate_ticks, cache_ticks = stats.cache_ticks,
            build_ticks = stats.build_ticks, hook_ticks = stats.hook_ticks,
            dropped_samples = stats.dropped, "tempo native commit measurement");
    });
}
