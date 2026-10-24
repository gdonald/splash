use splash::config::{Config, Options, Profiles, Settings};
use splash::external::PluginSet;
use splash::filter::Entry;
use splash::render_contents_settings;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

fn settings(jobs: usize) -> Settings {
    let options = Options {
        mode: Some("auto".to_string()),
        output: Some("ansi".to_string()),
        jobs: Some(jobs),
        ..Options::default()
    };

    Settings::resolve(
        &options,
        &Config::default(),
        &Profiles::new(PathBuf::from("unused")),
    )
    .unwrap()
}

/// Each sample log, repeated until it is long enough to be split across
/// worker threads
fn long_samples() -> Vec<(String, String)> {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("examples");
    let mut names: Vec<String> = fs::read_dir(&directory)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .filter(|name| name.ends_with(".log"))
        .collect();
    names.sort();

    names
        .into_iter()
        .filter_map(|name| {
            let text = fs::read_to_string(directory.join(&name)).ok()?;
            let lines = text.lines().count().max(1);
            let copies = (splash::PARALLEL_THRESHOLD + 1).div_ceil(lines);

            Some((name, text.repeat(copies)))
        })
        .collect()
}

/// Every fourth sample, for the tests that render the samples many times
fn some_samples() -> Vec<(String, String)> {
    long_samples().into_iter().step_by(4).collect()
}

fn render(text: &str, settings: &Settings) -> String {
    render_contents_settings(text, settings, &mut Entry::default())
}

#[test]
fn samples_render_the_same_across_worker_threads_as_on_one() {
    let one = settings(1);
    let four = settings(4);

    for (name, text) in long_samples().into_iter().step_by(2) {
        assert_eq!(render(&text, &four), render(&text, &one), "{}", name);
    }
}

#[test]
fn samples_rendered_on_several_threads_at_once_match_those_rendered_one_by_one() {
    let samples = some_samples();
    let settings = settings(2);
    let expected: Vec<String> = samples
        .iter()
        .map(|(_, text)| render(text, &settings))
        .collect();

    let rendered: Vec<String> = thread::scope(|scope| {
        let workers: Vec<_> = samples
            .iter()
            .map(|(_, text)| scope.spawn(|| render(text, &settings)))
            .collect();

        workers
            .into_iter()
            .map(|worker| worker.join().unwrap())
            .collect()
    });

    for ((name, _), (rendered, expected)) in samples.iter().zip(rendered.iter().zip(&expected)) {
        assert_eq!(rendered, expected, "{}", name);
    }
}

#[test]
fn rendering_goes_on_while_plugin_files_are_loaded_again_and_again() {
    let directory = tempfile::TempDir::new().unwrap();
    let plugin = directory.path().join("concurrency.toml");
    fs::write(
        &plugin,
        "name = \"concurrency-plugin\"\n[patterns]\nnever = '^line splash never reads$'\n",
    )
    .unwrap();

    let samples = some_samples();
    let settings = settings(2);
    let expected: Vec<String> = samples
        .iter()
        .map(|(_, text)| render(text, &settings))
        .collect();
    let rendering = AtomicBool::new(true);

    let rendered: Vec<String> = thread::scope(|scope| {
        scope.spawn(|| {
            let mut plugins = PluginSet::new();

            while rendering.load(Ordering::SeqCst) {
                assert!(plugins.load(std::slice::from_ref(&plugin)).is_empty());
                plugins.load(&[]);
            }
        });

        let rendered = samples
            .iter()
            .map(|(_, text)| render(text, &settings))
            .collect();

        rendering.store(false, Ordering::SeqCst);
        rendered
    });

    assert_eq!(rendered, expected);
}
