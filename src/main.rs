mod ax;
mod screen;

use std::process::Command;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};

use crate::screen::Region;

const WINDOW_WAIT_TIMEOUT: Duration = Duration::from_secs(10);
const WINDOW_POLL_INTERVAL: Duration = Duration::from_millis(50);

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();

    match arg_refs.as_slice() {
        ["trust"] => cmd_trust(),
        ["list", app] => cmd_list(app),
        ["place", app, region] => cmd_place(app, parse_region(region)?),
        ["launch", app, region] => cmd_launch(app, parse_region(region)?),
        ["demo2", a, b] => cmd_demo(&[(a, Region::LeftHalf), (b, Region::RightHalf)]),
        ["demo3", a, b, c] => cmd_demo(&[
            (a, Region::LeftThird),
            (b, Region::CenterThird),
            (c, Region::RightThird),
        ]),
        ["demo4", a, b, c, d] => cmd_demo(&[
            (a, Region::TopLeft),
            (b, Region::TopRight),
            (c, Region::BottomLeft),
            (d, Region::BottomRight),
        ]),
        _ => {
            eprintln!(
                "usage:\n  window-layout-poc trust\n  window-layout-poc list <app>\n  \
                 window-layout-poc place <app> <region>\n  window-layout-poc launch <app> <region>\n  \
                 window-layout-poc demo2 <a> <b>\n  window-layout-poc demo3 <a> <b> <c>\n  \
                 window-layout-poc demo4 <a> <b> <c> <d>\n\nregions: {}",
                Region::ALL_LABELS
            );
            std::process::exit(2);
        }
    }
}

fn parse_region(label: &str) -> Result<Region> {
    Region::parse(label).ok_or_else(|| {
        anyhow!(
            "unknown region {label:?}; expected one of: {}",
            Region::ALL_LABELS
        )
    })
}

/// Stage 1: does macOS consider this process trusted for AX control?
fn cmd_trust() -> Result<()> {
    let trusted = ax::is_trusted_with_prompt();
    println!("[trust] accessibility trusted: {trusted}");
    if !trusted {
        println!(
            "[trust] grant access in System Settings → Privacy & Security → Accessibility, then re-run"
        );
    }
    Ok(())
}

/// Stage 2: enumerate an app's windows to prove read access works.
fn cmd_list(app_name: &str) -> Result<()> {
    let pid = find_pid(app_name)?;
    println!("[list] {app_name} pid={pid}");
    let app = ax::application_element(pid);
    let windows = ax::windows(&app).context("failed to read AXWindows")?;
    println!("[list] {} window(s)", windows.len());
    for (index, window) in windows.iter().enumerate() {
        let title = ax::window_title(window).unwrap_or_else(|| "<untitled>".into());
        println!("[list]   {index}: {title}");
    }
    Ok(())
}

/// Stage 3: move an already-running app's front window into a region.
fn cmd_place(app_name: &str, region: Region) -> Result<()> {
    let pid = find_pid(app_name)?;
    let placed = place_front_window(pid, region)?;
    println!("[place] {app_name} → {region:?} ({placed})");
    Ok(())
}

/// Stage 4: the product path — launch, wait for the window, place it.
/// Measures every phase because routine latency is a PRD concern.
fn cmd_launch(app_name: &str, region: Region) -> Result<()> {
    let started = Instant::now();
    let status = Command::new("open").args(["-a", app_name]).status()?;
    if !status.success() {
        bail!("open -a {app_name} exited with {status}");
    }
    let opened_ms = started.elapsed().as_millis();

    let pid = wait_for_pid(app_name)?;
    let pid_ms = started.elapsed().as_millis();

    let window = wait_for_front_window(pid)?;
    let window_ms = started.elapsed().as_millis();

    let placement = ax::set_window_frame(&window, region.frame(screen::main_display_frame()))?;
    let placed_ms = started.elapsed().as_millis();

    println!(
        "[launch] {app_name} → {region:?} ({placement:?})  open={opened_ms}ms pid={pid_ms}ms window={window_ms}ms placed={placed_ms}ms"
    );
    Ok(())
}

/// Stages 5-6: full split layouts with several apps at once.
fn cmd_demo(placements: &[(&&str, Region)]) -> Result<()> {
    for (app_name, region) in placements {
        cmd_launch(app_name, *region)?;
    }
    println!("[demo] done — check the screen layout");
    Ok(())
}

fn place_front_window(pid: i32, region: Region) -> Result<String> {
    let app = ax::application_element(pid);
    let windows = ax::windows(&app).context("failed to read AXWindows")?;
    let window = windows
        .first()
        .ok_or_else(|| anyhow!("app has no windows"))?;
    let title = ax::window_title(window).unwrap_or_else(|| "<untitled>".into());
    let placement = ax::set_window_frame(window, region.frame(screen::main_display_frame()))?;
    Ok(format!("{title}, {placement:?}"))
}

fn find_pid(app_name: &str) -> Result<i32> {
    let output = Command::new("pgrep").args(["-x", app_name]).output()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .next()
        .and_then(|line| line.trim().parse().ok())
        .ok_or_else(|| anyhow!("no running process named {app_name:?} (is the app open?)"))
}

fn wait_for_pid(app_name: &str) -> Result<i32> {
    let deadline = Instant::now() + WINDOW_WAIT_TIMEOUT;
    loop {
        if let Ok(pid) = find_pid(app_name) {
            return Ok(pid);
        }
        if Instant::now() >= deadline {
            bail!("timed out waiting for {app_name} process");
        }
        std::thread::sleep(WINDOW_POLL_INTERVAL);
    }
}

fn wait_for_front_window(pid: i32) -> Result<ax::AxElement> {
    let app = ax::application_element(pid);
    let deadline = Instant::now() + WINDOW_WAIT_TIMEOUT;
    loop {
        if let Ok(mut windows) = ax::windows(&app) {
            if !windows.is_empty() {
                return Ok(windows.remove(0));
            }
        }
        if Instant::now() >= deadline {
            bail!("timed out waiting for a window of pid {pid}");
        }
        std::thread::sleep(WINDOW_POLL_INTERVAL);
    }
}
