extern crate clap;
extern crate serde_json;

use clap::{App, Arg};
use serde_json::Value;
use std::env;
use std::process::Command;

fn build_deps(is_release: bool, target: Option<String>) {
    let output = Command::new("cargo")
        .args(&["build", "--build-plan", "-Z", "unstable-options"])
        .output()
        .expect("Failed to execute");

    if !output.status.success() {
        let stderr = String::from_utf8(output.stderr).expect("Not UTF-8");
        panic!(stderr)
    }

    let plan = String::from_utf8(output.stdout).expect("Not UTF-8");
    let cwd = env::current_dir().unwrap();
    let val: Value = serde_json::from_str(&plan).unwrap();
    let invocations = val.get("invocations").unwrap().as_array().unwrap();

    let pkgs: Vec<String> = invocations
        .iter()
        .filter(|&x| {
            x.get("args").unwrap().as_array().unwrap().len() != 0
                && x.get("cwd").unwrap().as_str().unwrap() != cwd.as_os_str()
        })
        .map(|ref x| {
            let env = x.get("env").unwrap().as_object().unwrap();
            let name = env.get("CARGO_PKG_NAME").unwrap().as_str().unwrap();
            let version = env.get("CARGO_PKG_VERSION").unwrap().as_str().unwrap();
            format!("{}:{}", name, version)
        })
        .collect();

    let mut command = Command::new("cargo");
    command.arg("build");

    for pkg in pkgs {
        command.args(&["-p", &pkg]);
    }

    if is_release {
        command.arg("--release");
    }

    if let Some(target) = target {
        command.args(&["--target", &target]);
    }

    execute_command(&mut command);
}

fn main() {
    let matched_args = App::new("cargo build-deps")
        .arg(Arg::with_name("build-deps"))
        .arg(Arg::with_name("release").long("release"))
        .arg(Arg::with_name("target").long("target"))
        .get_matches();

    let is_release = matched_args.is_present("release");
    let target = matched_args.value_of("target");

    build_deps(is_release, target.map(ToOwned::to_owned));
}

fn execute_command(command: &mut Command) {
    let mut child = command
        .envs(env::vars())
        .spawn()
        .expect("failed to execute process");

    let exit_status = child.wait().expect("failed to run command");

    if !exit_status.success() {
        match exit_status.code() {
            Some(code) => panic!("Exited with status code: {}", code),
            None => panic!("Process terminated by signal"),
        }
    }
}
