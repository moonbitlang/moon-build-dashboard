use std::{io::Write, string::FromUtf8Error};

#[derive(Debug, thiserror::Error)]
#[error("moon operations error: {cmd}")]
pub struct MoonOpsError {
    cmd: String,
    #[source]
    kind: MoonOpsErrorKind,
}

#[derive(Debug, thiserror::Error)]
pub enum MoonOpsErrorKind {
    #[error("non-zero exit code: {0}")]
    ReturnNonZero(std::process::ExitStatus),
    #[error("io error")]
    IOError(#[from] std::io::Error),
    #[error("utf8 error")]
    FromUtf8Error(#[from] FromUtf8Error),
}

pub fn get_moon_version() -> Result<String, MoonOpsError> {
    let cmd = "moon version";
    let output = std::process::Command::new("moon")
        .args(["version"])
        .output()
        .map_err(|e| MoonOpsError {
            cmd: cmd.to_string(),
            kind: MoonOpsErrorKind::IOError(e),
        })?;
    if !output.status.success() {
        return Err(MoonOpsError {
            cmd: cmd.to_string(),
            kind: MoonOpsErrorKind::ReturnNonZero(output.status),
        });
    }
    let version = String::from_utf8(output.stdout).map_err(|e| MoonOpsError {
        cmd: cmd.to_string(),
        kind: MoonOpsErrorKind::FromUtf8Error(e),
    })?;
    Ok(version.trim().to_string())
}

pub fn get_moonc_version() -> Result<String, MoonOpsError> {
    let cmd = "moonc -v";
    let output = std::process::Command::new("moonc")
        .args(["-v"])
        .output()
        .map_err(|e| MoonOpsError {
            cmd: cmd.to_string(),
            kind: MoonOpsErrorKind::IOError(e),
        })?;
    if !output.status.success() {
        return Err(MoonOpsError {
            cmd: cmd.to_string(),
            kind: MoonOpsErrorKind::ReturnNonZero(output.status),
        });
    }
    let version = String::from_utf8(output.stdout).map_err(|e| MoonOpsError {
        cmd: cmd.to_string(),
        kind: MoonOpsErrorKind::FromUtf8Error(e),
    })?;
    Ok(version.trim().to_string())
}

fn install_unix_release(args: &[&str]) -> Result<(), MoonOpsError> {
    let curl_cmd = "curl -fsSL https://cli.moonbitlang.com/install/unix.sh";
    let output = std::process::Command::new("curl")
        .args(["-fsSL", "https://cli.moonbitlang.com/install/unix.sh"])
        .output()
        .map_err(|e| MoonOpsError {
            cmd: curl_cmd.to_string(),
            kind: MoonOpsErrorKind::IOError(e),
        })?;
    if !output.status.success() {
        return Err(MoonOpsError {
            cmd: curl_cmd.to_string(),
            kind: MoonOpsErrorKind::ReturnNonZero(output.status),
        });
    }

    let bash_cmd = format!("bash {}", args.join(" "));
    let mut cmd = std::process::Command::new("bash")
        .args(args)
        .stdin(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| MoonOpsError {
            cmd: bash_cmd.to_string(),
            kind: MoonOpsErrorKind::IOError(e),
        })?;

    if let Some(stdin) = cmd.stdin.as_mut() {
        stdin.write_all(&output.stdout).map_err(|e| MoonOpsError {
            cmd: bash_cmd.to_string(),
            kind: MoonOpsErrorKind::IOError(e),
        })?;
    }

    let status = cmd.wait().map_err(|e| MoonOpsError {
        cmd: bash_cmd.to_string(),
        kind: MoonOpsErrorKind::IOError(e),
    })?;
    if !status.success() {
        return Err(MoonOpsError {
            cmd: bash_cmd.to_string(),
            kind: MoonOpsErrorKind::ReturnNonZero(status),
        });
    }

    let version_cmd = "moon version --all";
    let output = std::process::Command::new("moon")
        .args(["version", "--all"])
        .output()
        .map_err(|e| MoonOpsError {
            cmd: version_cmd.to_string(),
            kind: MoonOpsErrorKind::IOError(e),
        })?;
    if !output.status.success() {
        return Err(MoonOpsError {
            cmd: version_cmd.to_string(),
            kind: MoonOpsErrorKind::ReturnNonZero(output.status),
        });
    }
    println!("Version command output: {}", String::from_utf8_lossy(&output.stdout));

    Ok(())
}

fn install_windows_release(is_bleeding: bool) -> Result<(), MoonOpsError> {
    let cmd_str = "Set-ExecutionPolicy RemoteSigned -Scope CurrentUser; irm https://cli.moonbitlang.cn/install/powershell.ps1 | iex";
    let mut cmd = std::process::Command::new("powershell");
    cmd.args(["-Command", cmd_str]);
    
    if is_bleeding {
        cmd.env("MOONBIT_INSTALL_VERSION", "nightly");
    }

    let output = cmd.output().map_err(|e| MoonOpsError {
        cmd: cmd_str.to_string(),
        kind: MoonOpsErrorKind::IOError(e),
    })?;

    if !output.status.success() {
        return Err(MoonOpsError {
            cmd: cmd_str.to_string(),
            kind: MoonOpsErrorKind::ReturnNonZero(output.status),
        });
    }

    let version_cmd = "moon version --all";
    let output = std::process::Command::new("moon")
        .args(["version", "--all"])
        .output()
        .map_err(|e| MoonOpsError {
            cmd: version_cmd.to_string(),
            kind: MoonOpsErrorKind::IOError(e),
        })?;
    if !output.status.success() {
        return Err(MoonOpsError {
            cmd: version_cmd.to_string(),
            kind: MoonOpsErrorKind::ReturnNonZero(output.status),
        });
    }
    println!("Version command output: {}", String::from_utf8_lossy(&output.stdout));

    Ok(())
}


pub fn install_stable_release() -> Result<(), MoonOpsError> {
    #[cfg(unix)]
    let res = install_unix_release(&["-s"]);

    #[cfg(target_os = "windows")]
    let res = install_windows_release(false);

    res
}

pub fn install_bleeding_release() -> Result<(), MoonOpsError> {
    #[cfg(unix)]
    let res = install_unix_release(&["-s", "bleeding"]);

    #[cfg(target_os = "windows")]
    let res = install_windows_release(true);

    res
}

pub fn moon_update() -> Result<(), MoonOpsError> {
    let update_cmd = "moon update";
    let output = std::process::Command::new("moon")
        .args(["update"])
        .output()
        .map_err(|e| MoonOpsError {
            cmd: update_cmd.to_string(),
            kind: MoonOpsErrorKind::IOError(e),
        })?;
    if !output.status.success() {
        return Err(MoonOpsError {
            cmd: update_cmd.to_string(),
            kind: MoonOpsErrorKind::ReturnNonZero(output.status),
        });
    }
    Ok(())
}
