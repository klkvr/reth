use std::cell::RefCell;
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
    dropped: u64,
}
thread_local! { static STATS: RefCell<Stats> = RefCell::new(Stats::default()); }
pub(crate) fn start() -> Option<Stamp> {
    STATS.with(|stats| {
        let mut stats = stats.borrow_mut();
        stats.count += 1;
        (cfg!(target_arch = "x86_64") && stats.count % 64 == 0).then(Stamp::read)
    })
}
pub(crate) fn record(start: Option<Stamp>, middle: Option<Stamp>) {
    if let (Some(start), Some(middle)) = (start, middle) {
        let end = Stamp::read();
        STATS.with(|stats| {
            let mut stats = stats.borrow_mut();
            if let (Some(accumulate), Some(cache)) = (start.elapsed(middle), middle.elapsed(end)) {
                stats.samples += 1;
                stats.accumulate_ticks += accumulate;
                stats.cache_ticks += cache;
            } else {
                stats.dropped += 1;
            }
        });
    }
}
pub(crate) fn emit() {
    STATS.with(|stats| {
        let stats = std::mem::take(&mut *stats.borrow_mut());
        tracing::info!(target: "tempo_phase_measure", count = stats.count, samples = stats.samples,
            accumulate_ticks = stats.accumulate_ticks, cache_ticks = stats.cache_ticks,
            dropped_samples = stats.dropped, "tempo native commit measurement");
    });
}
