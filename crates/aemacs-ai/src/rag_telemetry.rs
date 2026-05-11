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

/// Telemetry data for a single memory category.
#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct CategoryMetrics {
    /// A rolling window of the last successful search scores.
    pub scores: VecDeque<f32>,
}

/// Global telemetry state for RAG search performance.
#[derive(Debug, Serialize, Deserialize, Default, Clone)]
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
    pub async fn new() -> AIResult<Self> {
        let home = std::env::var("HOME").map_err(|_| {
            crate::error::AIError::Unknown("HOME environment variable not set".to_string())
        })?;
        let file_path = PathBuf::from(home).join(".gemini").join("rag_metrics.json");

        let data = if file_path.exists() {
            let content = fs::read_to_string(&file_path)?;
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            RagTelemetryData::default()
        };

        Ok(Self { data: Arc::new(RwLock::new(data)), file_path })
    }

    /// Records a successful search score for a given silo.
    pub async fn log_score(&self, silo: &str, score: f32) {
        let mut data = self.data.write().await;
        let entry =
            data.categories.entry(silo.to_uppercase()).or_insert_with(CategoryMetrics::default);

        entry.scores.push_back(score);
        if entry.scores.len() > WINDOW_SIZE {
            entry.scores.pop_front();
        }

        // Auto-save on every log for now.
        if let Err(e) = self.save_internal(&data) {
            tracing::error!("Failed to save RAG telemetry: {e}");
        }
    }

    /// Retrieves the current similarity threshold for a given silo, considering overrides.
    pub async fn get_threshold(&self, silo: &str) -> f32 {
        let silo = silo.to_uppercase();
        let config = aemacs_core::config::get_config();

        // 1. Check User Config Overrides
        if let Some(overrides) = &config.rag_thresholds {
            let val = match silo.as_str() {
                "ARCHIVE" | "HISTORIC" => overrides.archive,
                "INSIGHT" => overrides.insight,
                "CORE" => overrides.core,
                "GENESIS" => overrides.genesis,
                _ => None,
            };
            if let Some(v) = val {
                return v;
            }
        }

        // 2. Check Calibrated Thresholds
        let data = self.data.read().await;
        if let Some(t) = data.thresholds.get(&silo) {
            return *t;
        }

        // 3. Fallback to hardcoded default (High Confidence)
        0.72
    }

    /// Starts the background calibration daemon ("The Night Shift").
    pub fn start_calibration_daemon(self: Arc<Self>) {
        tokio::spawn(async move {
            tracing::info!("🌙 [RAG] Calibration Daemon (The Night Shift) started.");
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(3600)); // 1 hour cycle
            loop {
                interval.tick().await;
                if let Err(e) = self.calibrate_thresholds().await {
                    tracing::error!("❌ [RAG] Calibration failed: {e}");
                }
            }
        });
    }

    /// Calculates and updates thresholds based on rolling window performance.
    pub async fn calibrate_thresholds(&self) -> AIResult<()> {
        let mut data = self.data.write().await;
        let config = aemacs_core::config::get_config();

        let mut updates = Vec::new();

        for (silo, metrics) in &data.categories {
            // Check for manual overrides - Skip calibration if set
            let has_override = if let Some(overrides) = &config.rag_thresholds {
                match silo.as_str() {
                    "ARCHIVE" | "HISTORIC" => overrides.archive.is_some(),
                    "INSIGHT" => overrides.insight.is_some(),
                    "CORE" => overrides.core.is_some(),
                    "GENESIS" => overrides.genesis.is_some(),
                    _ => false,
                }
            } else {
                false
            };

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

        self.save_internal(&data)?;
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

/// Calculates the ideal threshold using the 99th percentile of error (MSE proxy).
/// This represents a threshold that would have accepted 99% of successful historical hits.
fn calculate_ideal_threshold(scores: &VecDeque<f32>) -> f32 {
    let mut sorted_errors: Vec<f32> = scores.iter().map(|&s| 1.0 - s).collect();
    sorted_errors.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    // 99th percentile of error (the "worst" acceptable hit)
    let idx = (sorted_errors.len() as f32 * 0.99) as usize;
    // idx will be 99 for a window of 100.
    let p99_error = sorted_errors.get(idx).cloned().unwrap_or(0.40);

    1.0 - p99_error
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]
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
            thresholds: [("CORE".to_string(), 0.80)].into_iter().collect(),
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
}
