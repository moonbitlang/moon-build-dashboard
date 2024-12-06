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
        MooncakeSource, Status, ToolChainLabel, ToolChainVersion, CBT,
    },
    mooncakesio,
    util::{
        get_moon_version, get_moonc_version, install_bleeding_release, install_stable_release,
        MoonOpsError,
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
        .map_err(|e| RunMoonError::IOError(e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

    let elapsed = start.elapsed();

    eprintln!(
        "{}",
        format!(
            "moon {}, elapsed: {}ms",
            args.join(" ").blue().bold(),
            elapsed.as_millis()
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
    let db = mooncakesio::get_all_mooncakes().map_err(|e| GetMooncakeSourcesError {
        kind: GetMooncakeSourcesErrorKind::MooncakesIO(e),
    })?;
    let mut repo_list = vec![];
    if let Some(r) = &cmd.repo_url {
        repo_list.push(MooncakeSource::Git {
            url: r.clone(),
            rev: vec![],
            index: 0,
        });
    }

    if let Some(file) = &cmd.file {
        let content = std::fs::read_to_string(file).map_err(|e| GetMooncakeSourcesError {
            kind: GetMooncakeSourcesErrorKind::IOError(e),
        })?;
        for line in content.lines() {
            let s = line.trim();
            if s.starts_with("#") || s.trim().is_empty() {
                continue;
            } else if s.starts_with("https://") {
                // https://github.com/moonbitlang/core
                // https://github.com/moonbitlang/core hash1 hash2 hash3
                let parts: Vec<&str> = s.split(' ').collect();
                if parts.len() == 1 {
                    repo_list.push(MooncakeSource::Git {
                        url: parts[0].to_string(),
                        rev: vec!["HEAD".to_string()],
                        index: repo_list.len(),
                    });
                } else {
                    repo_list.push(MooncakeSource::Git {
                        url: parts[0].to_string(),
                        rev: parts[1..].iter().copied().map(|s| s.to_string()).collect(),
                        index: repo_list.len(),
                    });
                }
            } else {
                // moonbitlang/core
                // moonbitlang/core 0.1.0 0.2.0
                let parts: Vec<&str> = s.split(' ').collect();
                let name = parts[0].to_string();
                let mut xs: Vec<String> =
                    parts[1..].iter().copied().map(|s| s.to_string()).collect();
                if xs.is_empty() {
                    xs.push("latest".to_string());
                }
                if !db.contains_key(&name) {
                    eprintln!("{} not found", name);
                    continue;
                }
                let mut version: Vec<String> = xs
                    .iter()
                    .map(|s| {
                        if s == "latest" {
                            db.get_latest_version(&name)
                                .map_err(|e| GetMooncakeSourcesError {
                                    kind: GetMooncakeSourcesErrorKind::MooncakesDB(e),
                                })
                        } else {
                            Ok(s.to_string())
                        }
                    })
                    .collect::<Result<Vec<String>, GetMooncakeSourcesError>>()?;
                version.sort();
                version.dedup();
                repo_list.push(MooncakeSource::MooncakesIO {
                    name,
                    version,
                    index: repo_list.len(),
                });
            }
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

    let r = run_moon(workdir, source, &cmd.args()).map_err(|e| StatMooncakeError::RunMoon(e));
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
    let tmp = tempfile::tempdir().map_err(|e| BuildError::IOError(e))?;
    let mut cbts = vec![];

    match source {
        MooncakeSource::Git { url, rev, index: _ } => {
            git::git_clone_to(url, tmp.path(), "test").map_err(|e| BuildError::GitError(e))?;
            let workdir = tmp.path().join("test");
            for h in rev {
                if let Err(e) = git::git_checkout(&workdir, h) {
                    eprintln!("Failed to checkout {}: {}", h, e);
                    cbts.push(None);
                    continue;
                }
                cbts.push(run_matrix(&workdir, source).ok());
            }
        }
        MooncakeSource::MooncakesIO {
            name,
            version,
            index: _,
        } => {
            for v in version {
                if let Err(e) = mooncakesio::download_to(name, v, tmp.path()) {
                    eprintln!("Failed to download {}/{}: {}", name, v, e);
                    cbts.push(None);
                    continue;
                }
                let workdir = tmp.path().join(v);
                cbts.push(run_matrix(&workdir, source).ok());
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

fn run_matrix(workdir: &Path, source: &MooncakeSource) -> Result<CBT, RunMatrixError> {
    let check_wasm = stat_mooncake(workdir, source, MoonCommand::Check(Backend::Wasm))
        .map_err(|e| RunMatrixError::StatMooncake(e))?;
    let check_wasm_gc = stat_mooncake(workdir, source, MoonCommand::Check(Backend::WasmGC))
        .map_err(|e| RunMatrixError::StatMooncake(e))?;
    let check_js = stat_mooncake(workdir, source, MoonCommand::Check(Backend::Js))
        .map_err(|e| RunMatrixError::StatMooncake(e))?;

    let build_wasm = stat_mooncake(workdir, source, MoonCommand::Build(Backend::Wasm))
        .map_err(|e| RunMatrixError::StatMooncake(e))?;
    let build_wasm_gc = stat_mooncake(workdir, source, MoonCommand::Build(Backend::WasmGC))
        .map_err(|e| RunMatrixError::StatMooncake(e))?;
    let build_js = stat_mooncake(workdir, source, MoonCommand::Build(Backend::Js))
        .map_err(|e| RunMatrixError::StatMooncake(e))?;

    let test_wasm = stat_mooncake(workdir, source, MoonCommand::Test(Backend::Wasm))
        .map_err(|e| RunMatrixError::StatMooncake(e))?;
    let test_wasm_gc = stat_mooncake(workdir, source, MoonCommand::Test(Backend::WasmGC))
        .map_err(|e| RunMatrixError::StatMooncake(e))?;
    let test_js = stat_mooncake(workdir, source, MoonCommand::Test(Backend::Js))
        .map_err(|e| RunMatrixError::StatMooncake(e))?;

    Ok(CBT {
        check: BackendState {
            wasm: check_wasm,
            wasm_gc: check_wasm_gc,
            js: check_js,
        },
        build: BackendState {
            wasm: build_wasm,
            wasm_gc: build_wasm_gc,
            js: build_js,
        },
        test: BackendState {
            wasm: test_wasm,
            wasm_gc: test_wasm_gc,
            js: test_js,
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

    for source in mooncake_sources {
        let build_state = build(&source).map_err(|e| StatError {
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

    let mooncake_sources = get_mooncake_sources(&cmd).map_err(|e| StatError {
        kind: StatErrorKind::GetMooncakeSourcesError(e),
    })?;
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
    let os = "macos";
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
