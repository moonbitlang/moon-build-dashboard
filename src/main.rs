use std::{
    io::Write,
    path::Path,
    time::{Duration, Instant},
};

use chrono::{FixedOffset, Local};

use clap::Parser;
use colored::Colorize;
use flate2::{write::GzEncoder, Compression};
use moon_dashboard::{
    cli,
    dashboard::{
        Backend, BackendState, BuildState, ExecuteResult, MoonBuildDashboard, MoonCommand,
        MooncakeSource, Status, ToolChainLabel, ToolChainVersion, CBT, OS,
    },
    mooncakesio,
    util::{
        get_moon_version, get_moonc_version, get_repos_config, install_bleeding_release,
        install_stable_release, MoonOpsError,
    },
};
use moon_dashboard::{git, util::moon_update};

#[derive(Debug, thiserror::Error)]
pub enum RunMoonError {
    #[error("io error")]
    IOError(#[from] std::io::Error),

    #[error("non-zero exit code: {0}")]
    ReturnNonZero(std::process::ExitStatus),

    #[error("from utf8 error")]
    FromUtf8(#[from] std::string::FromUtf8Error),
}

#[derive(Debug)]
struct CommandOutput {
    duration: Duration,
    stdout: String,
    stderr: String,
    success: bool,
}

fn run_moon(
    workdir: &Path,
    source: &MooncakeSource,
    args: &[&str],
) -> Result<CommandOutput, RunMoonError> {
    let start = Instant::now();
    eprintln!(
        "{}",
        format!("RUN moon {} for {:?}", args.join(" "), source)
            .blue()
            .bold()
    );

    let output = std::process::Command::new("moon")
        .current_dir(workdir)
        .args(args)
        .output()
        .map_err(RunMoonError::IOError)?;

    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

    let elapsed = start.elapsed();

    eprintln!(
        "{}",
        format!(
            "moon {}, elapsed: {}ms, {}",
            args.join(" ").blue().bold(),
            elapsed.as_millis(),
            if output.status.success() {
                "success"
            } else {
                "failed"
            }
        )
        .green()
        .bold()
    );

    Ok(CommandOutput {
        duration: elapsed,
        stdout,
        stderr,
        success: output.status.success(),
    })
}

#[derive(Debug, thiserror::Error)]
#[error("get mooncake sources error")]
struct GetMooncakeSourcesError {
    #[source]
    kind: GetMooncakeSourcesErrorKind,
}

#[derive(Debug, thiserror::Error)]
enum GetMooncakeSourcesErrorKind {
    #[error("io error")]
    IOError(#[from] std::io::Error),
    #[error("failed on mooncakesio")]
    MooncakesIO(#[from] mooncakesio::MooncakesIOError),

    #[error("failed on mooncakesdb")]
    MooncakesDB(#[from] mooncakesio::MooncakesDBError),
}

fn get_mooncake_sources(
    cmd: &cli::StatSubcommand,
) -> Result<Vec<MooncakeSource>, GetMooncakeSourcesError> {
    let mut repo_list = vec![];
    let default_running_os = vec![OS::Linux, OS::MacOS, OS::Windows];
    let default_running_backend = vec![Backend::WasmGC, Backend::Wasm, Backend::Js, Backend::Native];

    if let Some(r) = &cmd.repo_url {
        repo_list.push(MooncakeSource::Git {
            url: r.clone(),
            rev: vec![],
            index: 0,
            running_os: default_running_os.clone(),
            running_backend: default_running_backend.clone(),
        });
    }

    if let Some(path) = &cmd.file {
        let repos = get_repos_config(path);
        let github_repos = repos.github_repos;
        let mooncakes = repos.mooncakes;
        for repo in github_repos {
            repo_list.push(MooncakeSource::Git {
                url: repo.link,
                rev: vec![repo.branch],
                index: repo_list.len(),
                running_os: repo.running_os.unwrap_or(default_running_os.clone()),
                running_backend: repo
                    .running_backend
                    .unwrap_or(default_running_backend.clone()),
            });
        }

        for mooncake in mooncakes {
            repo_list.push(MooncakeSource::MooncakesIO {
                name: mooncake.name,
                version: vec![mooncake.version],
                running_os: mooncake.running_os.unwrap_or(default_running_os.clone()),
                running_backend: mooncake
                    .running_backend
                    .unwrap_or(default_running_backend.clone()),
                index: repo_list.len(),
            });
        }
    }

    Ok(repo_list)
}

#[derive(Debug, thiserror::Error)]
enum StatMooncakeError {
    #[error("run moon")]
    RunMoon(#[from] RunMoonError),
}

fn stat_mooncake(
    workdir: &Path,
    source: &MooncakeSource,
    cmd: MoonCommand,
) -> Result<ExecuteResult, StatMooncakeError> {
    let _ = run_moon(workdir, source, &["clean"]);

    let is_moonbit_community = match source {
        MooncakeSource::MooncakesIO { name, .. } => name.contains("moonbitlang"),
        MooncakeSource::Git { url, .. } => url.contains("moonbitlang"),
    };

    let r = run_moon(workdir, source, &cmd.args(is_moonbit_community))
        .map_err(StatMooncakeError::RunMoon);
    let status = match r.as_ref() {
        Ok(output) if output.success => Status::Success,
        _ => Status::Failure,
    };
    let output = r.ok();
    let start_time = Local::now()
        .with_timezone(&FixedOffset::east_opt(8 * 3600).unwrap())
        .format("%Y-%m-%d %H:%M:%S.%3f")
        .to_string();
    let elapsed = output
        .as_ref()
        .map(|d| d.duration.as_millis() as u64)
        .unwrap_or(0);
    let execute_result = ExecuteResult {
        status,
        start_time,
        elapsed,
        stdout: output
            .as_ref()
            .map(|d| d.stdout.clone())
            .unwrap_or_default(),
        stderr: output
            .as_ref()
            .map(|d| d.stderr.clone())
            .unwrap_or_default(),
    };
    Ok(execute_result)
}

#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    #[error("io error")]
    IOError(#[from] std::io::Error),
    #[error("return non zero")]
    ReturnNonZero(std::process::ExitStatus),
    #[error("from utf8")]
    FromUtf8(#[from] std::string::FromUtf8Error),
    #[error("git")]
    GitError(git::GitOpsError),
}

pub fn build(source: &MooncakeSource) -> Result<BuildState, BuildError> {
    let tmp = tempfile::tempdir().map_err(BuildError::IOError)?;
    let mut cbts = vec![];

    match source {
        MooncakeSource::Git {
            url,
            rev,
            index: _,
            running_os,
            running_backend,
        } => {
            git::git_clone_to(url, tmp.path(), "test").map_err(BuildError::GitError)?;
            let workdir = tmp.path().join("test");
            for h in rev {
                if let Err(e) = git::git_checkout(&workdir, h) {
                    eprintln!("Failed to checkout {}: {}", h, e);
                    cbts.push(None);
                    continue;
                }
                cbts.push(run_matrix(&workdir, source, running_os, running_backend).ok());
            }
        }
        MooncakeSource::MooncakesIO {
            name,
            version,
            index: _,
            running_os,
            running_backend,
        } => {
            for v in version {
                if let Err(e) = mooncakesio::download_to(name, v, tmp.path()) {
                    eprintln!("Failed to download {}/{}: {}", name, v, e);
                    cbts.push(None);
                    continue;
                }
                let workdir = tmp.path().join(v);
                cbts.push(run_matrix(&workdir, source, running_os, running_backend).ok());
            }
        }
    }

    Ok(BuildState {
        source: source.get_index(),
        cbts,
    })
}

#[derive(Debug, thiserror::Error)]
enum RunMatrixError {
    #[error("stat mooncake")]
    StatMooncake(#[from] StatMooncakeError),
}

fn run_matrix(
    workdir: &Path,
    source: &MooncakeSource,
    running_os: &[OS],
    running_backend: &[Backend],
) -> Result<CBT, RunMatrixError> {
    let mut check_wasm = ExecuteResult::skip_result();
    let mut check_wasm_gc = ExecuteResult::skip_result();
    let mut check_js = ExecuteResult::skip_result();
    let mut check_native = ExecuteResult::skip_result();

    let mut build_wasm = ExecuteResult::skip_result();
    let mut build_wasm_gc = ExecuteResult::skip_result();
    let mut build_js = ExecuteResult::skip_result();
    let mut build_native = ExecuteResult::skip_result();

    let mut test_wasm = ExecuteResult::skip_result();
    let mut test_wasm_gc = ExecuteResult::skip_result();
    let mut test_js = ExecuteResult::skip_result();
    let mut test_native = ExecuteResult::skip_result();

    for os in running_os {
        match os {
            OS::Linux => {
                if cfg!(target_os = "linux") {
                    for backend in running_backend {
                        match backend {
                            Backend::Wasm => {
                                check_wasm = stat_mooncake(
                                    workdir,
                                    source,
                                    MoonCommand::Check(Backend::Wasm),
                                )
                                .map_err(RunMatrixError::StatMooncake)?;
                                build_wasm = stat_mooncake(
                                    workdir,
                                    source,
                                    MoonCommand::Build(Backend::Wasm),
                                )
                                .map_err(RunMatrixError::StatMooncake)?;
                                test_wasm = stat_mooncake(
                                    workdir,
                                    source,
                                    MoonCommand::Test(Backend::Wasm),
                                )
                                .map_err(RunMatrixError::StatMooncake)?;
                            }
                            Backend::WasmGC => {
                                check_wasm_gc = stat_mooncake(
                                    workdir,
                                    source,
                                    MoonCommand::Check(Backend::WasmGC),
                                )
                                .map_err(RunMatrixError::StatMooncake)?;
                                build_wasm_gc = stat_mooncake(
                                    workdir,
                                    source,
                                    MoonCommand::Build(Backend::WasmGC),
                                )
                                .map_err(RunMatrixError::StatMooncake)?;
                                test_wasm_gc = stat_mooncake(
                                    workdir,
                                    source,
                                    MoonCommand::Test(Backend::WasmGC),
                                )
                                .map_err(RunMatrixError::StatMooncake)?;
                            }
                            Backend::Js => {
                                check_js =
                                    stat_mooncake(workdir, source, MoonCommand::Check(Backend::Js))
                                        .map_err(RunMatrixError::StatMooncake)?;
                                build_js =
                                    stat_mooncake(workdir, source, MoonCommand::Build(Backend::Js))
                                        .map_err(RunMatrixError::StatMooncake)?;
                                test_js =
                                    stat_mooncake(workdir, source, MoonCommand::Test(Backend::Js))
                                        .map_err(RunMatrixError::StatMooncake)?;
                            }
                            Backend::Native => {
                                check_native =
                                    stat_mooncake(workdir, source, MoonCommand::Check(Backend::Native))
                                        .map_err(RunMatrixError::StatMooncake)?;
                                build_native =
                                    stat_mooncake(workdir, source, MoonCommand::Build(Backend::Native))
                                        .map_err(RunMatrixError::StatMooncake)?;
                                test_native =
                                    stat_mooncake(workdir, source, MoonCommand::Test(Backend::Native))
                                        .map_err(RunMatrixError::StatMooncake)?;
                            }
                        }
                    }
                }
            }
            OS::MacOS => {
                if cfg!(target_os = "macos") {
                    for backend in running_backend {
                        match backend {
                            Backend::Wasm => {
                                check_wasm = stat_mooncake(
                                    workdir,
                                    source,
                                    MoonCommand::Check(Backend::Wasm),
                                )
                                .map_err(RunMatrixError::StatMooncake)?;
                                build_wasm = stat_mooncake(
                                    workdir,
                                    source,
                                    MoonCommand::Build(Backend::Wasm),
                                )
                                .map_err(RunMatrixError::StatMooncake)?;
                                test_wasm = stat_mooncake(
                                    workdir,
                                    source,
                                    MoonCommand::Test(Backend::Wasm),
                                )
                                .map_err(RunMatrixError::StatMooncake)?;
                            }
                            Backend::WasmGC => {
                                check_wasm_gc = stat_mooncake(
                                    workdir,
                                    source,
                                    MoonCommand::Check(Backend::WasmGC),
                                )
                                .map_err(RunMatrixError::StatMooncake)?;
                                build_wasm_gc = stat_mooncake(
                                    workdir,
                                    source,
                                    MoonCommand::Build(Backend::WasmGC),
                                )
                                .map_err(RunMatrixError::StatMooncake)?;
                                test_wasm_gc = stat_mooncake(
                                    workdir,
                                    source,
                                    MoonCommand::Test(Backend::WasmGC),
                                )
                                .map_err(RunMatrixError::StatMooncake)?;
                            }
                            Backend::Js => {
                                check_js =
                                    stat_mooncake(workdir, source, MoonCommand::Check(Backend::Js))
                                        .map_err(RunMatrixError::StatMooncake)?;
                                build_js =
                                    stat_mooncake(workdir, source, MoonCommand::Build(Backend::Js))
                                        .map_err(RunMatrixError::StatMooncake)?;
                                test_js =
                                    stat_mooncake(workdir, source, MoonCommand::Test(Backend::Js))
                                        .map_err(RunMatrixError::StatMooncake)?;
                            }
                            Backend::Native => {
                                check_native =
                                    stat_mooncake(workdir, source, MoonCommand::Check(Backend::Native))
                                        .map_err(RunMatrixError::StatMooncake)?;
                                build_native =
                                    stat_mooncake(workdir, source, MoonCommand::Build(Backend::Native))
                                        .map_err(RunMatrixError::StatMooncake)?;
                                test_native =
                                    stat_mooncake(workdir, source, MoonCommand::Test(Backend::Native))
                                        .map_err(RunMatrixError::StatMooncake)?;
                            }
                        }
                    }
                }
            }
            OS::Windows => {
                if cfg!(target_os = "windows") {
                    for backend in running_backend {
                        match backend {
                            Backend::Wasm => {
                                check_wasm = stat_mooncake(
                                    workdir,
                                    source,
                                    MoonCommand::Check(Backend::Wasm),
                                )
                                .map_err(RunMatrixError::StatMooncake)?;
                                build_wasm = stat_mooncake(
                                    workdir,
                                    source,
                                    MoonCommand::Build(Backend::Wasm),
                                )
                                .map_err(RunMatrixError::StatMooncake)?;
                                test_wasm = stat_mooncake(
                                    workdir,
                                    source,
                                    MoonCommand::Test(Backend::Wasm),
                                )
                                .map_err(RunMatrixError::StatMooncake)?;
                            }
                            Backend::WasmGC => {
                                check_wasm_gc = stat_mooncake(
                                    workdir,
                                    source,
                                    MoonCommand::Check(Backend::WasmGC),
                                )
                                .map_err(RunMatrixError::StatMooncake)?;
                                build_wasm_gc = stat_mooncake(
                                    workdir,
                                    source,
                                    MoonCommand::Build(Backend::WasmGC),
                                )
                                .map_err(RunMatrixError::StatMooncake)?;
                                test_wasm_gc = stat_mooncake(
                                    workdir,
                                    source,
                                    MoonCommand::Test(Backend::WasmGC),
                                )
                                .map_err(RunMatrixError::StatMooncake)?;
                            }
                            Backend::Js => {
                                check_js =
                                    stat_mooncake(workdir, source, MoonCommand::Check(Backend::Js))
                                        .map_err(RunMatrixError::StatMooncake)?;
                                build_js =
                                    stat_mooncake(workdir, source, MoonCommand::Build(Backend::Js))
                                        .map_err(RunMatrixError::StatMooncake)?;
                                test_js =
                                    stat_mooncake(workdir, source, MoonCommand::Test(Backend::Js))
                                        .map_err(RunMatrixError::StatMooncake)?;
                            }
                            Backend::Native => {
                                check_native =
                                    stat_mooncake(workdir, source, MoonCommand::Check(Backend::Native))
                                        .map_err(RunMatrixError::StatMooncake)?;
                                build_native =
                                    stat_mooncake(workdir, source, MoonCommand::Build(Backend::Native))
                                        .map_err(RunMatrixError::StatMooncake)?;
                                test_native =
                                    stat_mooncake(workdir, source, MoonCommand::Test(Backend::Native))
                                        .map_err(RunMatrixError::StatMooncake)?;
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(CBT {
        check: BackendState {
            wasm: check_wasm,
            wasm_gc: check_wasm_gc,
            js: check_js,
            native: check_native,
        },
        build: BackendState {
            wasm: build_wasm,
            wasm_gc: build_wasm_gc,
            js: build_js,
            native: build_native,
        },
        test: BackendState {
            wasm: test_wasm,
            wasm_gc: test_wasm_gc,
            js: test_js,
            native: test_native,
        },
    })
}

#[derive(Debug, thiserror::Error)]
#[error("stat error")]
struct StatError {
    #[source]
    kind: StatErrorKind,
}

#[derive(Debug, thiserror::Error)]
enum StatErrorKind {
    #[error("failed on moon operations")]
    MoonOpsError(#[from] MoonOpsError),

    #[error("failed on get mooncake sources")]
    GetMooncakeSourcesError(#[from] GetMooncakeSourcesError),

    #[error("failed on build")]
    BuildError(#[from] BuildError),
}

fn stat(cmd: cli::StatSubcommand) -> Result<MoonBuildDashboard, StatError> {
    let run_id = std::env::var("GITHUB_ACTION_RUN_ID").unwrap_or("0".into());
    let run_number = std::env::var("GITHUB_ACTION_RUN_NUMBER").unwrap_or("0".into());

    if !cmd.skip_install {
        install_stable_release().map_err(|e| StatError {
            kind: StatErrorKind::MoonOpsError(e),
        })?;
    }
    if !cmd.skip_update {
        moon_update().map_err(|e| StatError {
            kind: StatErrorKind::MoonOpsError(e),
        })?;
    }
    let moon_version = get_moon_version().map_err(|e| StatError {
        kind: StatErrorKind::MoonOpsError(e),
    })?;
    let moonc_version = get_moonc_version().map_err(|e| StatError {
        kind: StatErrorKind::MoonOpsError(e),
    })?;
    let stable_toolchain_version = ToolChainVersion {
        label: ToolChainLabel::Stable,
        moon_version,
        moonc_version,
    };

    let mooncake_sources = get_mooncake_sources(&cmd).map_err(|e| StatError {
        kind: StatErrorKind::GetMooncakeSourcesError(e),
    })?;
    let mut stable_release_data = vec![];

    for source in mooncake_sources.iter() {
        let build_state = build(source).map_err(|e| StatError {
            kind: StatErrorKind::BuildError(e),
        })?;
        stable_release_data.push(build_state);
    }

    if !cmd.skip_install {
        install_bleeding_release().map_err(|e| StatError {
            kind: StatErrorKind::MoonOpsError(e),
        })?;
    }
    if !cmd.skip_update {
        moon_update().map_err(|e| StatError {
            kind: StatErrorKind::MoonOpsError(e),
        })?;
    }
    let moon_version = get_moon_version().map_err(|e| StatError {
        kind: StatErrorKind::MoonOpsError(e),
    })?;
    let moonc_version = get_moonc_version().map_err(|e| StatError {
        kind: StatErrorKind::MoonOpsError(e),
    })?;
    let bleeding_toolchain_version = ToolChainVersion {
        label: ToolChainLabel::Bleeding,
        moon_version,
        moonc_version,
    };

    let mut bleeding_release_data = vec![];

    for source in mooncake_sources.iter() {
        let build_state = build(source).map_err(|e| StatError {
            kind: StatErrorKind::BuildError(e),
        })?;
        bleeding_release_data.push(build_state);
    }

    let result = MoonBuildDashboard {
        run_id,
        run_number,
        sources: mooncake_sources,
        start_time: Local::now().to_rfc3339(),
        stable_toolchain_version,
        stable_release_data,
        bleeding_toolchain_version,
        bleeding_release_data,
    };
    Ok(result)
}

fn main0() -> anyhow::Result<()> {
    let cli = cli::MoonBuildDashBoardCli::parse();
    let res = match cli.subcommand {
        cli::MoonBuildDashBoardSubcommands::Stat(cmd) => stat(cmd),
    };
    #[cfg(target_os = "windows")]
    let os = "windows";
    #[cfg(target_os = "linux")]
    let os = "linux";
    #[cfg(target_os = "macos")]
    let os = "mac";
    match res {
        Ok(dashboard) => {
            let date = Local::now().format("%Y-%m-%d");
            let filename = format!("webapp/public/{}/{}_data.jsonl.gz", os, date);

            let fp = std::fs::OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&filename)?;
            let encoder = GzEncoder::new(fp, Compression::default());
            let mut writer = std::io::BufWriter::new(encoder);
            writeln!(writer, "{}", serde_json::to_string(&dashboard)?)?;
            writer.flush()?;
            writer.into_inner()?.finish()?;

            let latest_filename = format!("webapp/public/{}/latest_data.jsonl.gz", os);
            std::fs::copy(&filename, latest_filename)?;

            Ok(())
        }
        Err(e) => Err(anyhow::anyhow!(e)),
    }
}

fn main() -> anyhow::Result<()> {
    main0()
}
