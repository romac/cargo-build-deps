use clap::{App, Arg, SubCommand};
use semver::Version;
use serde::Deserialize;
use std::env;
use std::fmt;
use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
struct BuildPlan {
    invocations: Vec<Invocation>,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
struct Invocation {
    args: Vec<String>,
    cwd: String,
    env: Env,
}

#[allow(non_snake_case)]
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
struct Env {
    CARGO_PKG_NAME: String,
    CARGO_PKG_VERSION: String,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Package {
    name: String,
    version: Version,
}

impl fmt::Display for Package {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        fmt.write_fmt(format_args!("{}:{}", self.name, self.version))
    }
}

fn build_deps(is_release: bool, manifest: Option<PathBuf>, target: Option<String>, debug: bool) {
    let mut cmd = Command::new("cargo");

    cmd.args(&["build", "--build-plan", "-Z", "unstable-options"]);

    if let Some(ref manifest) = manifest {
        cmd.args(&["--manifest-path", manifest.to_str().unwrap()]);
    }

    if debug {
        println!("[debug] Running command '{:?}'...", cmd);
    }

    let output = cmd.output().expect("Failed to execute");

    if !output.status.success() {
        let stderr = String::from_utf8(output.stderr).expect("Not UTF-8");
        panic!(stderr)
    }

    let build_plan_json = String::from_utf8(output.stdout).expect("Not UTF-8");

    let cwd = env::current_dir().unwrap();

    let build_plan: BuildPlan = serde_json::from_str(&build_plan_json).unwrap();

    let mut pkgs: Vec<Package> = build_plan
        .invocations
        .into_iter()
        .filter(|i| i.args.len() != 0 && i.cwd.as_str() != cwd.as_os_str())
        .map(|i| Package {
            name: i.env.CARGO_PKG_NAME,
            version: Version::parse(&i.env.CARGO_PKG_VERSION).unwrap(),
        })
        .collect();

    pkgs.sort();
    pkgs.reverse();
    pkgs.dedup_by_key(|p| p.name.clone());
    pkgs.reverse();

    let mut command = Command::new("cargo");
    command.arg("build");

    if is_release {
        command.arg("--release");
    }

    if let Some(target) = target {
        command.args(&["--target", &target]);
    }

    if let Some(manifest) = manifest {
        command.args(&["--manifest-path", manifest.to_str().unwrap()]);
    }

    for pkg in pkgs {
        command.args(&["-p", &pkg.to_string()]);
    }

    if debug {
        println!("[debug] Running command '{:?}'...", command);
    }

    execute_command(&mut command);
}

fn main() {
    let matches = App::new("cargo")
        .usage("cargo build-deps [FLAGS] [OPTIONS]")
        .subcommand(
            SubCommand::with_name("build-deps")
                .name("build-deps")
                .usage("cargo build-deps [FLAGS] [OPTIONS]")
                .arg(Arg::with_name("debug").short("d").long("debug"))
                .arg(Arg::with_name("release").long("release"))
                .arg(Arg::with_name("workspace").short("w").long("workspace"))
                .arg(
                    Arg::with_name("target")
                        .short("t")
                        .long("target")
                        .value_name("TARGET")
                        .takes_value(true),
                ),
        )
        .get_matches();

    if let Some(matches) = matches.subcommand_matches("build-deps") {
        let release = matches.is_present("release");
        let target = matches.value_of("target");
        let debug = matches.is_present("debug");
        let workspace = matches.is_present("workspace");

        if workspace {
            let manifest_contents = std::fs::read_to_string("Cargo.toml").unwrap();
            let manifest: toml::Value = toml::from_str(&manifest_contents).unwrap();
            let members: Vec<String> = manifest
                .get("workspace")
                .unwrap()
                .get("members")
                .unwrap()
                .clone()
                .try_into()
                .unwrap();

            let cwd = env::current_dir().unwrap();

            for member in members {
                let mut path = cwd.clone();
                path.push(&member);
                path.push("Cargo.toml");

                println!(
                    "[info] Building dependencies of workspace member '{}'...",
                    member
                );
                build_deps(release, Some(path), target.map(ToOwned::to_owned), debug);
                println!("[info] => DONE");
            }
        } else {
            println!("[info] Building dependencies...");
            build_deps(release, None, target.map(ToOwned::to_owned), debug);
            println!("[info] => DONE");
        }
    }
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
