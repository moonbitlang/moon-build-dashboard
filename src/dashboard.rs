use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub enum MooncakeSource {
    MooncakesIO {
        name: String,
        version: Vec<String>,
        running_os: Vec<OS>,
        running_backend: Vec<Backend>,
        index: usize,
    },
    Git {
        url: String,
        rev: Vec<String>,
        running_os: Vec<OS>,
        running_backend: Vec<Backend>,
        index: usize,
    },
}

impl MooncakeSource {
    pub fn get_index(&self) -> usize {
        match self {
            MooncakeSource::MooncakesIO { index, .. } => *index,
            MooncakeSource::Git { index, .. } => *index,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub enum Backend {
    #[serde(rename = "wasm")]
    Wasm,
    #[serde(rename = "wasm-gc")]
    WasmGC,
    #[serde(rename = "js")]
    Js,
}

impl Backend {
    pub fn to_flag(&self) -> &str {
        match self {
            Backend::Wasm => "wasm",
            Backend::WasmGC => "wasm-gc",
            Backend::Js => "js",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub enum OS {
    #[serde(rename = "linux")]
    Linux,
    #[serde(rename = "macos")]
    MacOS,
    #[serde(rename = "windows")]
    Windows,
}

impl OS {
    pub fn to_flag(&self) -> &str {
        match self {
            OS::Linux => "linux",
            OS::MacOS => "macos",
            OS::Windows => "windows",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum MoonCommand {
    Check(Backend),
    Build(Backend),
    Test(Backend),
}

impl MoonCommand {
    pub fn args(&self, is_moonbit_community: bool) -> Vec<&str> {
        match self {
            MoonCommand::Check(backend) => vec!["check", "-q", "--target", backend.to_flag()],
            MoonCommand::Build(backend) => vec!["build", "-q", "--target", backend.to_flag()],
            MoonCommand::Test(backend) => {
                // only run test for moonbit community project
                if is_moonbit_community {
                    vec!["test", "-q", "--target", backend.to_flag()]
                } else {
                    vec!["test", "-q", "--build-only", "--target", backend.to_flag()]
                }
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ToolChainLabel {
    Stable,
    Bleeding,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ToolChainVersion {
    pub label: ToolChainLabel,
    pub moon_version: String,
    pub moonc_version: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MoonBuildDashboard {
    pub run_id: String,
    pub run_number: String,
    pub start_time: String,

    pub sources: Vec<MooncakeSource>,

    pub stable_toolchain_version: ToolChainVersion,
    pub stable_release_data: Vec<BuildState>,

    pub bleeding_toolchain_version: ToolChainVersion,
    pub bleeding_release_data: Vec<BuildState>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Status {
    Success,
    Failure,
    Skipped,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExecuteResult {
    pub status: Status,
    pub start_time: String,
    pub elapsed: u64,
    pub stdout: String,
    pub stderr: String,
}

impl ExecuteResult {
    pub fn skip_result() -> Self {
        Self {
            status: Status::Skipped,
            start_time: "".to_string(),
            elapsed: 0,
            stdout: "".to_string(),
            stderr: "".to_string(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BackendState {
    pub wasm: ExecuteResult,
    pub wasm_gc: ExecuteResult,
    pub js: ExecuteResult,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CBT {
    pub check: BackendState,
    pub build: BackendState,
    pub test: BackendState,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BuildState {
    pub source: usize,
    pub cbts: Vec<Option<CBT>>,
}
