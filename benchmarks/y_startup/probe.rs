//! Deliberately scoped dispatch probe, not a replacement for Y.
use std::{
    env,
    process::{self, Command},
};

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if args == ["noop"] {
        return;
    }
    if args.len() != 2 || args[0] != "focus" {
        process::exit(2);
    }
    let direction = match args[1].as_str() {
        "left" => "west",
        "right" => "east",
        "up" => "north",
        "down" => "south",
        _ => process::exit(2),
    };
    let output = Command::new(env::var("Y_BENCH_YABAI").expect("fixture required"))
        .args(["-m", "window", "--focus", direction])
        .output()
        .expect("fixture execution failed");
    process::exit(output.status.code().unwrap_or(1));
}
