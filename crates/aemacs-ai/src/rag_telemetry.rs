use std::{
    collections::{HashMap, VecDeque},
    fs,
    path::PathBuf,
    sync::Arc,
};

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::AIResult;

/// Maximum number of scores to keep in the rolling window for statistical dampening.
const WINDOW_SIZE: usize = 1000;
/// Number of consecutive degraded recall outcomes required before maintenance is justified.
const MAINTENANCE_TRIGGER_STREAK: usize = 3;
/// Cooldown between maintenance nudges so weak recall does not thrash the database.
const MAINTENANCE_COOLDOWN_SECONDS: i64 = 900;

/// Coarse-grained retrieval outcome tiers used for degraded-recall tracking.
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RecallOutcome {
    High,
    Medium,
    Low,
    None,
    DiagnosticOnly,
}

impl RecallOutcome {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::High => "high",
            Self::Medium => "medium",
            Self::Low => "low",
            Self::None => "none",
            Self::DiagnosticOnly => "diagnostic_only",
        }
    }

    const fn is_degraded(self) -> bool {
        matches!(self, Self::Low | Self::None | Self::DiagnosticOnly)
    }
}

/// Telemetry data for a single memory category.
#[derive(Debug, Serialize, Deserialize, Default, Clone)]
#[serde(default)]
pub struct CategoryMetrics {
    /// A rolling window of the last successful search scores.
    pub scores: VecDeque<f32>,
    /// Recent recall outcomes for degraded-recall detection and maintenance decisions.
    pub recall_outcomes: VecDeque<RecallOutcome>,
    /// Unix timestamp of the last maintenance trigger for this silo.
    pub last_maintenance_at: Option<i64>,
}

/// Global telemetry state for RAG search performance.
#[derive(Debug, Serialize, Deserialize, Default, Clone)]
#[serde(default)]
pub struct RagTelemetryData {
    /// Metrics segregated by memory category (silos).
    pub categories: HashMap<String, CategoryMetrics>,
    /// Calibrated thresholds for each silo.
    pub thresholds: HashMap<String, f32>,
}

/// Utility for logging and persisting RAG search telemetry.
#[derive(Debug)]
pub struct RagTelemetry {
    data: Arc<RwLock<RagTelemetryData>>,
    file_path: PathBuf,
}

impl RagTelemetry {
    /// Creates a new instance of the telemetry logger, loading existing data if available.
    ///
    /// # Errors
    /// Returns an error if the HOME environment variable is missing or if the telemetry file
    /// cannot be read.
    pub async fn new() -> AIResult<Self> {
        let home = std::env::var("HOME").map_err(|_| {
            crate::error::AIError::Unknown("HOME environment variable not set".to_string())
        })?;
        let file_path = PathBuf::from(home).join(".gemini").join("rag_metrics.json");

        let data = if file_path.exists() {
            let content = tokio::fs::read_to_string(&file_path).await?;
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            RagTelemetryData::default()
        };

        Ok(Self { data: Arc::new(RwLock::new(data)), file_path })
    }

    /// Records a successful search score for a given silo.
    pub async fn log_score(&self, silo: &str, score: f32) {
        let snapshot = {
            let mut data = self.data.write().await;
            let entry =
                data.categories.entry(silo.to_uppercase()).or_insert_with(CategoryMetrics::default);

            entry.scores.push_back(score);
            if entry.scores.len() > WINDOW_SIZE {
                entry.scores.pop_front();
            }

            data.clone()
        };

        // Auto-save on every log for now.
        if let Err(e) = self.save_internal(&snapshot) {
            tracing::error!("Failed to save RAG telemetry: {e}");
        }
    }

    /// Records the qualitative outcome of a recall attempt and decides whether maintenance should
    /// be triggered for the affected silo.
    pub async fn record_recall_outcome(&self, silo: &str, outcome: RecallOutcome) -> bool {
        let (should_trigger_maintenance, snapshot) = {
            let mut data = self.data.write().await;
            let entry =
                data.categories.entry(silo.to_uppercase()).or_insert_with(CategoryMetrics::default);

            entry.recall_outcomes.push_back(outcome);
            if entry.recall_outcomes.len() > WINDOW_SIZE {
                entry.recall_outcomes.pop_front();
            }

            let degraded_streak = entry
                .recall_outcomes
                .iter()
                .rev()
                .take_while(|candidate| candidate.is_degraded())
                .count();
            let now = chrono::Utc::now().timestamp();
            let cooldown_elapsed = entry.last_maintenance_at.is_none_or(|last_maintenance_at| {
                now - last_maintenance_at >= MAINTENANCE_COOLDOWN_SECONDS
            });
            let should_trigger_maintenance = outcome.is_degraded()
                && degraded_streak >= MAINTENANCE_TRIGGER_STREAK
                && cooldown_elapsed;

            if should_trigger_maintenance {
                entry.last_maintenance_at = Some(now);
            }

            (should_trigger_maintenance, data.clone())
        };

        if let Err(e) = self.save_internal(&snapshot) {
            tracing::error!("Failed to save degraded recall telemetry: {e}");
        }

        should_trigger_maintenance
    }

    /// Retrieves the current similarity threshold for a given silo, considering overrides.
    pub async fn get_threshold(&self, silo: &str) -> f32 {
        let silo = silo.to_uppercase();
        let config = aemacs_core::config::get_config();

        // 1. Check User Config Overrides
        if let Some(v) = config.rag_thresholds.as_ref().and_then(|overrides| match silo.as_str() {
            "ARCHIVE" | "HISTORIC" => overrides.archive,
            "INSIGHT" => overrides.insight,
            "CORE" => overrides.core,
            "GENESIS" => overrides.genesis,
            _ => None,
        }) {
            return v;
        }

        // 2. Check Calibrated Thresholds
        if let Some(t) = {
            let data = self.data.read().await;
            data.thresholds.get(&silo).copied()
        } {
            return t;
        }

        // 3. Fallback to hardcoded default (High Confidence)
        0.72
    }

    /// Starts the background calibration daemon ("The Night Shift").
    pub fn start_calibration_daemon(self: Arc<Self>) {
        tokio::spawn(async move {
            tracing::info!("🌙 [RAG] Calibration Daemon (The Night Shift) started.");
            let mut interval = tokio::time::interval(tokio::time::Duration::from_hours(1));
            loop {
                interval.tick().await;
                if let Err(e) = self.calibrate_thresholds().await {
                    tracing::error!("❌ [RAG] Calibration failed: {e}");
                }
            }
        });
    }

    /// Calculates and updates thresholds based on rolling window performance.
    ///
    /// # Errors
    /// Returns an error if the recalibrated telemetry snapshot cannot be persisted to disk.
    pub async fn calibrate_thresholds(&self) -> AIResult<()> {
        let config = aemacs_core::config::get_config();

        let snapshot = {
            let mut data = self.data.write().await;
            let mut updates = Vec::new();

            for (silo, metrics) in &data.categories {
                // Check for manual overrides - Skip calibration if set
                let has_override =
                    config.rag_thresholds.as_ref().is_some_and(|overrides| match silo.as_str() {
                        "ARCHIVE" | "HISTORIC" => overrides.archive.is_some(),
                        "INSIGHT" => overrides.insight.is_some(),
                        "CORE" => overrides.core.is_some(),
                        "GENESIS" => overrides.genesis.is_some(),
                        _ => false,
                    });

                if has_override {
                    tracing::debug!(
                        "⏩ [RAG] Skipping calibration for {silo} (Manual Override active)"
                    );
                    continue;
                }

                if metrics.scores.len() < 10 {
                    // Not enough data for statistical significance
                    continue;
                }

                let current_threshold = *data.thresholds.get(silo).unwrap_or(&0.72);
                let ideal_threshold = calculate_ideal_threshold(&metrics.scores);

                // Safety Bounds: Max shift +/- 0.05 per cycle
                let diff = ideal_threshold - current_threshold;
                let clamped_diff = diff.clamp(-0.05, 0.05);
                let mut new_threshold = current_threshold + clamped_diff;

                // Hard Physical Floor: 0.60
                if new_threshold < 0.60 {
                    new_threshold = 0.60;
                }

                tracing::info!(
                    "⚖️ [RAG] Calibrated {silo}: {current_threshold:.3} -> {new_threshold:.3} (Ideal: {ideal_threshold:.3})"
                );
                updates.push((silo.clone(), new_threshold));
            }

            for (silo, val) in updates {
                data.thresholds.insert(silo, val);
            }

            data.clone()
        };

        self.save_internal(&snapshot)?;
        Ok(())
    }

    fn save_internal(&self, data: &RagTelemetryData) -> AIResult<()> {
        let content = serde_json::to_string_pretty(data).map_err(|e| {
            crate::error::AIError::Unknown(format!("Failed to serialize telemetry: {e}"))
        })?;

        if let Some(parent) = self.file_path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(&self.file_path, content)?;

        Ok(())
    }
}

#[cfg(test)]
impl RagTelemetry {
    pub(crate) async fn reset_silo_for_test(&self, silo: &str) {
        let mut data = self.data.write().await;
        let silo = silo.to_uppercase();
        data.categories.remove(&silo);
        data.thresholds.remove(&silo);
    }

    pub(crate) async fn set_threshold_for_test(&self, silo: &str, threshold: f32) {
        let mut data = self.data.write().await;
        data.thresholds.insert(silo.to_uppercase(), threshold);
    }
}

/// Calculates the ideal threshold using the 99th percentile of error (MSE proxy).
/// This represents a threshold that would have accepted 99% of successful historical hits.
fn calculate_ideal_threshold(scores: &VecDeque<f32>) -> f32 {
    let mut sorted_errors: Vec<f32> = scores.iter().map(|&s| 1.0 - s).collect();
    sorted_errors.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    // 99th percentile of error (the "worst" acceptable hit)
    let idx = sorted_errors.len().saturating_mul(99) / 100;
    // idx will be 99 for a window of 100.
    let p99_error = sorted_errors.get(idx).copied().unwrap_or(0.40);

    1.0 - p99_error
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::float_cmp, clippy::unwrap_used)]
    use tempfile::tempdir;

    use super::*;

    /// HARK! Verifying that the Calibration Daemon respects the 0.05 shift clamp. [R-CALIB-01]
    #[tokio::test]
    async fn test_calibration_clamping_and_floor() -> crate::AIResult<()> {
        let dir = tempdir().expect("Failed to create temp dir");
        let file_path = dir.path().join("rag_metrics.json");

        // 1. Setup telemetry with a known starting threshold of 0.80
        let data = RagTelemetryData {
            categories: HashMap::new(),
            thresholds: std::iter::once(("CORE".to_string(), 0.80)).collect(),
        };

        let telemetry =
            RagTelemetry { data: Arc::new(RwLock::new(data)), file_path: file_path.clone() };

        // 2. Simulate a "Dragon" of low scores (0.60) in the rolling window.
        // This should suggest an ideal threshold of 0.60, a drop of 0.20.
        for _ in 0..20 {
            telemetry.log_score("CORE", 0.60).await;
        }

        // 3. Trigger calibration
        telemetry.calibrate_thresholds().await?;

        // 4. Verify Clamping: 0.80 - 0.05 = 0.75 (NOT 0.60)
        let new_threshold = telemetry.get_threshold("CORE").await;
        assert!(
            (new_threshold - 0.75).abs() < f32::EPSILON,
            "Threshold shift was not clamped to 0.05! Got: {new_threshold}"
        );

        // 5. Verify Floor: Run it many times to hit the 0.60 floor
        for _ in 0..10 {
            telemetry.calibrate_thresholds().await?;
        }
        let floor_threshold = telemetry.get_threshold("CORE").await;
        assert!(
            floor_threshold >= 0.60,
            "Threshold fell below the physical floor of 0.60! Got: {floor_threshold}"
        );

        Ok(())
    }

    /// HARK! Verifying that statistical precision is calculated correctly.
    #[test]
    fn test_ideal_threshold_calculation() {
        let mut scores = VecDeque::new();
        // 99 hits at 0.90, 1 hit at 0.70
        for _ in 0..99 {
            scores.push_back(0.90);
        }
        scores.push_back(0.70);

        let ideal = calculate_ideal_threshold(&scores);
        // The 99th percentile of error is (1.0 - 0.70) = 0.30.
        // Ideal = 1.0 - 0.30 = 0.70.
        assert!((ideal - 0.70).abs() < f32::EPSILON, "Ideal calculation failed! Got: {ideal}");
    }

    /// HARK! Verifying the priority of threshold retrieval. [R-CALIB-03]
    #[tokio::test]
    async fn test_get_threshold_priority() -> crate::AIResult<()> {
        let dir = tempdir().expect("Failed to create temp dir");
        let file_path = dir.path().join("rag_metrics_priority.json");

        let mut data = RagTelemetryData::default();
        data.thresholds.insert("CORE".to_string(), 0.88); // Calibrated

        let telemetry = RagTelemetry { data: Arc::new(RwLock::new(data)), file_path };

        let threshold = telemetry.get_threshold("CORE").await;

        // Verify that the calibrated value is returned when no override is present.
        assert_eq!(threshold, 0.88, "Calibrated value should be returned when no override exists.");

        let default_threshold = telemetry.get_threshold("INSIGHT").await;
        assert_eq!(
            default_threshold, 0.72,
            "Default value should be returned when no calibration or override exists."
        );

        Ok(())
    }

    /// HARK! A lone degraded omen must not summon maintenance. [R-RECALL-01]
    #[tokio::test]
    async fn test_single_degraded_outcome_does_not_trigger_maintenance() {
        let dir = tempdir().expect("Failed to create temp dir");
        let telemetry = RagTelemetry {
            data: Arc::new(RwLock::new(RagTelemetryData::default())),
            file_path: dir.path().join("rag_metrics_single_degraded.json"),
        };

        let triggered = telemetry.record_recall_outcome("CORE", RecallOutcome::Low).await;

        assert!(!triggered, "A single degraded recall must not trigger maintenance.");
    }

    /// HARK! Three degraded omens in succession must awaken the maintenance drums. [R-RECALL-02]
    #[tokio::test]
    async fn test_repeated_degraded_outcomes_trigger_maintenance_at_streak_threshold() {
        let dir = tempdir().expect("Failed to create temp dir");
        let telemetry = RagTelemetry {
            data: Arc::new(RwLock::new(RagTelemetryData::default())),
            file_path: dir.path().join("rag_metrics_repeated_degraded.json"),
        };

        assert!(!telemetry.record_recall_outcome("CORE", RecallOutcome::Low).await);
        assert!(!telemetry.record_recall_outcome("CORE", RecallOutcome::None).await);
        assert!(telemetry.record_recall_outcome("CORE", RecallOutcome::DiagnosticOnly).await);
    }

    /// HARK! Once maintenance is summoned, cooldown bars immediate re-summoning. [R-RECALL-03]
    #[tokio::test]
    async fn test_maintenance_trigger_is_rate_limited_by_cooldown() {
        let dir = tempdir().expect("Failed to create temp dir");
        let telemetry = RagTelemetry {
            data: Arc::new(RwLock::new(RagTelemetryData::default())),
            file_path: dir.path().join("rag_metrics_cooldown.json"),
        };

        assert!(!telemetry.record_recall_outcome("CORE", RecallOutcome::Low).await);
        assert!(!telemetry.record_recall_outcome("CORE", RecallOutcome::Low).await);
        assert!(telemetry.record_recall_outcome("CORE", RecallOutcome::Low).await);
        assert!(
            !telemetry.record_recall_outcome("CORE", RecallOutcome::Low).await,
            "Cooldown should block an immediate second maintenance trigger."
        );
    }

    /// HARK! A strong hit must shatter the degraded streak. [R-RECALL-04]
    #[tokio::test]
    async fn test_non_degraded_outcome_breaks_degraded_streak() {
        let dir = tempdir().expect("Failed to create temp dir");
        let telemetry = RagTelemetry {
            data: Arc::new(RwLock::new(RagTelemetryData::default())),
            file_path: dir.path().join("rag_metrics_break_streak.json"),
        };

        assert!(!telemetry.record_recall_outcome("CORE", RecallOutcome::Low).await);
        assert!(!telemetry.record_recall_outcome("CORE", RecallOutcome::Low).await);
        assert!(
            !telemetry.record_recall_outcome("CORE", RecallOutcome::High).await,
            "High confidence should break the degraded streak."
        );
        assert!(!telemetry.record_recall_outcome("CORE", RecallOutcome::Low).await);
        assert!(
            !telemetry.record_recall_outcome("CORE", RecallOutcome::Low).await,
            "After a high-confidence break, the streak must restart from zero."
        );

        assert!(
            !telemetry.record_recall_outcome("CORE", RecallOutcome::Medium).await,
            "Medium confidence must also count as non-degraded."
        );
        assert!(!telemetry.record_recall_outcome("CORE", RecallOutcome::DiagnosticOnly).await);
        assert!(!telemetry.record_recall_outcome("CORE", RecallOutcome::None).await);
    }
}
