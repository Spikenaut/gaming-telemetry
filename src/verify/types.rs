use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimVerdict {
    Pass,
    Fail,
    Inconclusive,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModState {
    ActiveInstalled,
    DownloadedOnly,
    PartiallyInstalledBroken,
    Missing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SettingsSource {
    Persisted,
    Defaults,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OverallStatus {
    Pass,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileEvidence {
    pub path: String,
    pub source_label: String,
    pub size_bytes: u64,
    pub mtime: Option<String>,
    pub readable: bool,
    pub is_symlink: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ErrorDetail {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestMetadata {
    pub install_root: Option<String>,
    pub steam_buildid: Option<String>,
    pub last_updated: Option<String>,
    pub last_played: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GameSection {
    pub search_paths: Vec<FileEvidence>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_game_root: Option<FileEvidence>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_manifest: Option<FileEvidence>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_proton_prefix: Option<FileEvidence>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BinaryEvidence {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<FileEvidence>,
    pub verdict: ClaimVerdict,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InstallSection {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub install_root: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub steam_buildid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_updated: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_played: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manifest: Option<FileEvidence>,
    pub required_binaries: BTreeMap<String, BinaryEvidence>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConfigSection {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_confidence: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_file: Option<FileEvidence>,
    pub values: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModArtifact {
    pub state: ModState,
    pub verdict: ClaimVerdict,
    pub evidence: Vec<FileEvidence>,
    pub forensic_evidence: Vec<FileEvidence>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModsSection {
    pub artifacts: BTreeMap<String, ModArtifact>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrowdSettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub multiplier: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shuffle_amount: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disable_lq_crowds: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrowdProfileSection {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_crowd_density: Option<String>,
    pub high_crowd_profile: ClaimVerdict,
    pub settings_source: SettingsSource,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ultra_plus_crowds_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ultra_plus_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nova_crowds: Option<CrowdSettings>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PathTracingSection {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ray_tracing: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ray_traced_path_tracing: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ray_traced_path_tracing_for_photo_mode: Option<bool>,
    pub ultra_plus_mode: String,
    pub verdict: ClaimVerdict,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DlssSection {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolution_scaling: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dlss_backend_preset: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dlss: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dlss_d: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dlss_frame_gen: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dlss_multi_frame_generation: Option<String>,
    pub dlss_4_download_verified: ClaimVerdict,
    pub dlss_transformer_selected: ClaimVerdict,
    pub dlss_upscaling_enabled: ClaimVerdict,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuntimeThresholds {
    pub avg_gpu_w: f64,
    pub max_gpu_w: f64,
    pub max_vram_mb: f64,
    pub max_pcie_rx_mb_s: f64,
    /// Minimum average CPU package power (W) that indicates active crowd-AI workload.
    /// Nova Crowds drives dense NPC simulation on the CPU; values below this suggest
    /// the crowd system is inactive or the session was captured outside a populated area.
    pub avg_cpu_w: f64,
}

impl Default for RuntimeThresholds {
    fn default() -> Self {
        Self {
            avg_gpu_w: 200.0,
            max_gpu_w: 280.0,
            max_vram_mb: 10_000.0,
            max_pcie_rx_mb_s: 1_000.0,
            avg_cpu_w: 80.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuntimeMetrics {
    pub avg_gpu_w: f64,
    pub max_gpu_w: f64,
    pub avg_vram_mb: f64,
    pub max_vram_mb: f64,
    pub avg_pcie_rx_mb_s: f64,
    pub max_pcie_rx_mb_s: f64,
    /// Average CPU package power in watts. Present only when the telemetry source
    /// includes a `cpu_package_power_w` column, which captures the burst load driven
    /// by Nova Crowds' NPC-AI and traversal simulation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avg_cpu_w: Option<f64>,
    /// Peak CPU package power in watts across the captured session.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_cpu_w: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuntimeSection {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub telemetry_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_type: Option<String>,
    pub thresholds: RuntimeThresholds,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metrics: Option<RuntimeMetrics>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime_parse_error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime_corroboration: Option<ClaimVerdict>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Sidecar {
    pub schema_version: String,
    pub generated_at: String,
    pub workload_profile: String,
    pub game: GameSection,
    pub install: InstallSection,
    pub config: ConfigSection,
    pub mods: ModsSection,
    pub crowd_profile: CrowdProfileSection,
    pub path_tracing: PathTracingSection,
    pub dlss: DlssSection,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime: Option<RuntimeSection>,
    pub verdict: BTreeMap<String, ClaimVerdict>,
    pub overall_status: OverallStatus,
    pub error_details: Vec<ErrorDetail>,
}
