use splash::bench::sample_log;
use splash::config::{Config, Options, Profiles, Settings};
use splash::filter::Entry;
use splash::stats::Stats;
use std::alloc::{GlobalAlloc, Layout, System};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

/// The system allocator, counting the bytes allocated now and the most
/// allocated at once since the count was last reset
struct Counting;

static ALLOCATED: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);

/// Tests in this file run one at a time, since they share the counts
static MEASURING: Mutex<()> = Mutex::new(());

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc(layout) };

        if !pointer.is_null() {
            ALLOCATIONS.fetch_add(1, Ordering::SeqCst);
            let now = ALLOCATED.fetch_add(layout.size(), Ordering::SeqCst) + layout.size();
            PEAK.fetch_max(now, Ordering::SeqCst);
        }

        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) };
        ALLOCATED.fetch_sub(layout.size(), Ordering::SeqCst);
    }
}

#[global_allocator]
static COUNTING: Counting = Counting;

/// The most bytes allocated at once while `work` ran, beyond what was
/// allocated before it
fn peak_during(work: impl FnOnce()) -> usize {
    let before = ALLOCATED.load(Ordering::SeqCst);
    PEAK.store(before, Ordering::SeqCst);

    work();

    PEAK.load(Ordering::SeqCst) - before
}

fn settings(grep: &[&str]) -> Settings {
    let options = Options {
        mode: Some("clf".to_string()),
        output: Some("plain".to_string()),
        jobs: Some(1),
        filter: splash::filter::FilterOptions {
            grep: grep.iter().map(|pattern| pattern.to_string()).collect(),
            ..Default::default()
        },
        ..Options::default()
    };

    Settings::resolve(
        &options,
        &Config::default(),
        &Profiles::new(PathBuf::from("unused")),
    )
    .unwrap()
}

/// The allocations `work` made
fn allocations_during(work: impl FnOnce()) -> usize {
    let before = ALLOCATIONS.load(Ordering::SeqCst);

    work();

    ALLOCATIONS.load(Ordering::SeqCst) - before
}

fn render_in_chunks(text: &str, settings: &Settings) {
    let mut entry = Entry::default();
    let mut stats = Stats::default();

    for chunk in splash::source::line_chunks(text, splash::source::CHUNK_BYTES) {
        splash::render_contents_counted(chunk, settings, &mut entry, &mut stats);
    }
}

fn ansi_settings() -> Settings {
    let options = Options {
        mode: Some("clf".to_string()),
        output: Some("ansi".to_string()),
        jobs: Some(1),
        ..Options::default()
    };

    Settings::resolve(
        &options,
        &Config::default(),
        &Profiles::new(PathBuf::from("unused")),
    )
    .unwrap()
}

/// Allocations each line rendered in ANSI may make. Raise it only for a
/// change that needs the allocations.
const ANSI_ALLOCATIONS_PER_LINE: usize = 80;

/// Allocations each line rendered as plain text may make
const PLAIN_ALLOCATIONS_PER_LINE: usize = 40;

#[test]
fn rendering_a_file_in_chunks_holds_as_much_memory_whatever_its_size() {
    let _measuring = MEASURING.lock().unwrap();
    let settings = ansi_settings();
    let three_megabytes = sample_log(30_000);
    let ten_megabytes = sample_log(120_000);

    let smaller = peak_during(|| render_in_chunks(&three_megabytes, &settings));
    let larger = peak_during(|| render_in_chunks(&ten_megabytes, &settings));

    assert!(larger < smaller * 2, "{} then {}", smaller, larger);
    assert!(
        larger < 8 * splash::source::CHUNK_BYTES,
        "{} bytes held at once",
        larger
    );
}

#[test]
fn rendering_a_file_read_in_chunks_holds_as_much_memory_whatever_its_size() {
    let _measuring = MEASURING.lock().unwrap();
    let settings = ansi_settings();
    let directory = tempfile::TempDir::new().unwrap();
    let three_megabytes = directory.path().join("three.log");
    let ten_megabytes = directory.path().join("ten.log");

    std::fs::write(&three_megabytes, sample_log(30_000)).unwrap();
    std::fs::write(&ten_megabytes, sample_log(120_000)).unwrap();

    let render = |path: &std::path::Path| {
        let mut entry = Entry::default();
        let mut stats = Stats::default();

        let encoding = splash::source::Encoding::Utf8;

        splash::source::read_chunks(path, splash::source::CHUNK_BYTES, encoding, |chunk| {
            splash::render_contents_counted(chunk, &settings, &mut entry, &mut stats);
        })
        .unwrap();
    };

    let smaller = peak_during(|| render(&three_megabytes));
    let larger = peak_during(|| render(&ten_megabytes));

    assert!(larger < smaller * 2, "{} then {}", smaller, larger);
    assert!(
        larger < 8 * splash::source::CHUNK_BYTES,
        "{} bytes held at once",
        larger
    );
}

#[test]
fn rendering_a_line_in_ansi_makes_no_more_allocations_than_its_ceiling() {
    let _measuring = MEASURING.lock().unwrap();
    let settings = ansi_settings();
    let text = sample_log(10_000);

    let allocations = allocations_during(|| render_in_chunks(&text, &settings));

    assert!(
        allocations <= ANSI_ALLOCATIONS_PER_LINE * 10_000,
        "{} allocations per line",
        allocations as f64 / 10_000.0
    );
}

#[test]
fn rendering_a_line_as_plain_text_makes_no_more_allocations_than_its_ceiling() {
    let _measuring = MEASURING.lock().unwrap();
    let settings = settings(&[]);
    let text = sample_log(10_000);

    let allocations = allocations_during(|| render_in_chunks(&text, &settings));

    assert!(
        allocations <= PLAIN_ALLOCATIONS_PER_LINE * 10_000,
        "{} allocations per line",
        allocations as f64 / 10_000.0
    );
}

#[test]
fn a_filtered_out_line_holds_no_rendering() {
    let _measuring = MEASURING.lock().unwrap();
    let text = sample_log(20_000);
    let rendered_settings = settings(&[]);
    let filtered_settings = settings(&["no line holds this"]);

    let rendered = peak_during(|| render_in_chunks(&text, &rendered_settings));
    let filtered = peak_during(|| render_in_chunks(&text, &filtered_settings));

    assert!(filtered <= rendered, "{} then {}", rendered, filtered);
}

/// The bytes still allocated after `work` returned, beyond those allocated
/// before it. With one worker this is zero from the second render on. Worker
/// threads fill the regex crate's per-thread cache pool, which is capped, and
/// how soon it fills depends on which threads the runs land on, so with
/// several workers a render can still keep a few bytes after warming up.
fn retained_after(work: impl FnOnce()) -> isize {
    let before = ALLOCATED.load(Ordering::SeqCst) as isize;

    work();

    ALLOCATED.load(Ordering::SeqCst) as isize - before
}

#[test]
fn rendering_again_and_again_releases_everything_it_allocates() {
    let _measuring = MEASURING.lock().unwrap();
    let text = format!(
        "{}{}",
        sample_log(1_000),
        std::fs::read_to_string(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/examples/mixed_stream.log")
        )
        .unwrap()
    );

    for output in ["ansi", "plain", "json", "jsonl", "csv", "html"] {
        let options = Options {
            mode: Some("auto".to_string()),
            output: Some(output.to_string()),
            jobs: Some(1),
            ..Options::default()
        };
        let settings = Settings::resolve(
            &options,
            &Config::default(),
            &Profiles::new(PathBuf::from("unused")),
        )
        .unwrap();

        render_in_chunks(&text, &settings);

        let retained: Vec<isize> = (0..3)
            .map(|_| retained_after(|| render_in_chunks(&text, &settings)))
            .collect();

        assert_eq!(retained, [0, 0, 0], "{}", output);
    }
}

#[test]
fn worker_threads_stop_holding_more_memory_once_their_caches_are_full() {
    let _measuring = MEASURING.lock().unwrap();
    let text = sample_log(1_500);
    let options = Options {
        mode: Some("ad-hoc".to_string()),
        output: Some("ansi".to_string()),
        jobs: Some(4),
        ..Options::default()
    };
    let settings = Settings::resolve(
        &options,
        &Config::default(),
        &Profiles::new(PathBuf::from("unused")),
    )
    .unwrap();

    let first = retained_after(|| render_in_chunks(&text, &settings));

    for _ in 0..30 {
        render_in_chunks(&text, &settings);
    }

    let later: isize = (0..10)
        .map(|_| retained_after(|| render_in_chunks(&text, &settings)))
        .sum();

    assert!(
        later < first,
        "the first render kept {} bytes and the ten after warming up kept {}",
        first,
        later
    );
}
