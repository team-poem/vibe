use anyhow::{Context, Result};
use vibe_poc_action_runner::action::Action;
use vibe_poc_action_runner::runner::{Stats, measure};

const DEFAULT_WARMUP: usize = 1;
const DEFAULT_REPS: usize = 5;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [] => run_suite(),
        [kind, param] => run_single(kind, param),
        _ => {
            eprintln!("{}", usage());
            std::process::exit(2);
        }
    }
}

fn run_suite() -> Result<()> {
    let actions = [
        Action::osascript("return 1"),
        Action::open_app("Calculator"),
        Action::open_url("https://example.com"),
    ];
    println!(
        "[suite] warmup={DEFAULT_WARMUP}  repetitions={DEFAULT_REPS}  (run with `<kind> <param>` to measure one action)"
    );
    print_header();
    for action in &actions {
        let stats = measure(action, DEFAULT_REPS, DEFAULT_WARMUP)?;
        print_stats(&stats);
    }
    Ok(())
}

fn run_single(kind: &str, param: &str) -> Result<()> {
    let action = Action::parse(kind, param)
        .with_context(|| format!("unknown action kind '{kind}'\n{}", usage()))?;
    println!(
        "[single] kind={kind}  param={param}  warmup={DEFAULT_WARMUP}  repetitions={DEFAULT_REPS}"
    );
    print_header();
    let stats = measure(&action, DEFAULT_REPS, DEFAULT_WARMUP)?;
    print_stats(&stats);
    Ok(())
}

fn usage() -> String {
    String::from(
        "usage:\n  \
         vibe-poc-action-runner                       # run built-in suite\n  \
         vibe-poc-action-runner <kind> <param>        # measure one action\n\
         \n\
         kinds: open-app | open-url | osascript | shortcut\n\
         example: vibe-poc-action-runner shortcut test-shortcut",
    )
}

fn print_header() {
    println!(
        "{:<11}  {:>7} {:>7} {:>7} {:>7}   {:>8} {:>8} {:>8} {:>8}   success",
        "kind", "sp_min", "sp_p50", "sp_p95", "sp_max", "dis_min", "dis_p50", "dis_p95", "dis_max",
    );
}

fn print_stats(stats: &Stats) {
    println!(
        "{:<11}  {:>7.2} {:>7.2} {:>7.2} {:>7.2}   {:>8.2} {:>8.2} {:>8.2} {:>8.2}   {}/{}",
        stats.action.kind_label(),
        stats.spawn.min,
        stats.spawn.p50,
        stats.spawn.p95,
        stats.spawn.max,
        stats.dispatch.min,
        stats.dispatch.p50,
        stats.dispatch.p95,
        stats.dispatch.max,
        stats.success_count,
        stats.total,
    );
}
