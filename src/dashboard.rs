use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub enum MooncakeSource {
    MooncakesIO {
        name: String,
        version: Vec<String>,
        index: usize,
    },
    Git {
        url: String,
        rev: Vec<String>,
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum Backend {
    Wasm,
    WasmGC,
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum MoonCommand {
    Check(Backend),
    Build(Backend),
    Test(Backend),
}

impl MoonCommand {
    pub fn args(&self) -> Vec<&str> {
        match self {
            MoonCommand::Check(backend) => vec!["check", "-q", "--target", backend.to_flag()],
            MoonCommand::Build(backend) => vec!["build", "-q", "--target", backend.to_flag()],
            MoonCommand::Test(backend) => {
                vec!["test", "-q", "--build-only", "--target", backend.to_flag()]
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
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExecuteResult {
    pub status: Status,
    pub start_time: String,
    pub elapsed: u64,
    pub stdout: String,
    pub stderr: String,
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
