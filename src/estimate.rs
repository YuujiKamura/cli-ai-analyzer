//! Geometry-calibrated volume estimation for dump truck cargo.
//!
//! Detects tailgate boundaries and cargo top via AI, converts to meters using
//! tailgate height as scale reference, then estimates fill ratios separately.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::{analyze, AnalyzeOptions, OutputFormat};

/// Shape factor: peak height overestimates volume because cargo is mound-shaped.
/// Empirical correction ~0.85 (between flat-top=1.0 and cone=0.33).
const SHAPE_FACTOR: f64 = 0.85;

/// Truck bed specifications (length, width, height in meters)
#[derive(Debug, Clone)]
pub struct TruckSpec {
    pub bed_length: f64,
    pub bed_width: f64,
    pub bed_height: f64,
}

/// Get truck spec by class name
fn get_truck_spec(truck_class: &str) -> Option<TruckSpec> {
    match truck_class {
        "2t" => Some(TruckSpec { bed_length: 3.0, bed_width: 1.6, bed_height: 0.32 }),
        "4t" => Some(TruckSpec { bed_length: 3.4, bed_width: 2.06, bed_height: 0.32 }),
        "増トン" => Some(TruckSpec { bed_length: 4.0, bed_width: 2.2, bed_height: 0.40 }),
        "10t" => Some(TruckSpec { bed_length: 5.3, bed_width: 2.3, bed_height: 0.50 }),
        _ => None,
    }
}

/// Get material density (t/m3) by name
fn get_material_density(material: &str) -> Option<f64> {
    match material {
        "土砂" => Some(1.8),
        "As殻" => Some(2.5),
        "Co殻" => Some(2.5),
        "開粒度As殻" => Some(2.35),
        _ => None,
    }
}

/// Options for estimate command
#[derive(Debug, Clone)]
pub struct EstimateOptions {
    pub truck_class: String,
    pub material: String,
    pub ensemble: usize,
    pub analyze_options: AnalyzeOptions,
}

/// Result of estimation
#[derive(Debug, Clone, Serialize)]
pub struct EstimateResult {
    pub method: String,
    pub truck_class: String,
    pub material_type: String,
    pub height_m: f64,
    pub fill_ratio_l: f64,
    pub fill_ratio_w: f64,
    pub packing_density: f64,
    pub estimated_volume_m3: f64,
    pub estimated_tonnage: f64,
    pub density: f64,
    pub reasoning: String,
}

/// Geometry detection prompt
const GEOMETRY_PROMPT: &str = concat!(
    r#"Output ONLY JSON: {"plateBox":[x1,y1,x2,y2], "tailgateTopY": 0.0, "tailgateBottomY": 0.0, "cargoTopY": 0.0} "#,
    "This is a rear view of a dump truck carrying construction debris. ",
    "plateBox = bounding box of the rear license plate (normalized 0-1, [left,top,right,bottom]). ",
    "tailgateTopY = Y coordinate (normalized 0-1) of the TOP edge of the tailgate (後板上端/rim). ",
    "tailgateBottomY = Y coordinate (normalized 0-1) of the BOTTOM edge of the tailgate (後板下端). ",
    "cargoTopY = Y coordinate (normalized 0-1) of the HIGHEST point of the cargo pile. ",
    "The tailgate is the flat metal panel at the rear of the truck bed. ",
    "tailgateTopY < tailgateBottomY < plateBox[3] (top has smaller Y). ",
    "cargoTopY < tailgateTopY if cargo is heaped above the rim. ",
    "cargoTopY > tailgateTopY if cargo is below the rim. ",
    "All coordinates normalized 0.0-1.0.",
);

/// Fill ratio estimation prompt
const FILL_PROMPT: &str = concat!(
    r#"Output ONLY JSON: {"fillRatioL": 0.0, "fillRatioW": 0.0, "packingDensity": 0.0, "reasoning": "..."} "#,
    "This is a rear view of a dump truck carrying construction debris (As殻 = asphalt chunks). ",
    "Estimate each parameter INDEPENDENTLY: ",
    "fillRatioL (0.3~0.9): fraction of the bed LENGTH occupied by cargo. ",
    "Dump trucks are loaded from above; cargo forms a mound that rarely reaches the very front/rear. ",
    "Full load with cargo touching both ends = 0.85-0.9. Normal load = 0.6-0.8. Light load = 0.4-0.6. ",
    "fillRatioW (0.5~1.0): fraction of the bed WIDTH covered by cargo at the top surface. ",
    "Usually 0.8-1.0 since cargo spreads across the width. ",
    "packingDensity (0.5~0.9): how tightly packed the debris chunks are. ",
    "As殻 = asphalt pavement chunks (~5cm thick). ",
    "Large chunks thrown in loosely with visible gaps = 0.5-0.6, moderate = 0.65-0.7, tightly packed = 0.8-0.9.",
);

/// JSON response from geometry detection
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeometryResponse {
    plate_box: Option<Vec<f64>>,
    tailgate_top_y: Option<f64>,
    tailgate_bottom_y: Option<f64>,
    cargo_top_y: Option<f64>,
}

/// JSON response from fill estimation
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FillResponse {
    fill_ratio_l: Option<f64>,
    fill_ratio_w: Option<f64>,
    packing_density: Option<f64>,
    reasoning: Option<String>,
}

/// Parse JSON from AI response, stripping markdown code blocks if present
fn parse_json_response(raw: &str) -> Result<serde_json::Value> {
    let trimmed = raw.trim();

    // Find the first { and last }
    let start = trimmed.find('{');
    let end = trimmed.rfind('}');

    match (start, end) {
        (Some(s), Some(e)) if e > s => {
            let json_str = &trimmed[s..=e];
            serde_json::from_str(json_str)
                .map_err(|e| Error::GeminiError(format!("JSON parse error: {}", e)))
        }
        _ => serde_json::from_str(trimmed)
            .map_err(|e| Error::GeminiError(format!("JSON parse error: {}", e))),
    }
}

/// Run geometry-calibrated volume estimation
pub fn estimate(image_path: &Path, options: EstimateOptions) -> Result<EstimateResult> {
    // Look up truck spec
    let truck = get_truck_spec(&options.truck_class).ok_or_else(|| {
        Error::GeminiError(format!("Unknown truck class: {}", options.truck_class))
    })?;

    // Look up material density
    let density = get_material_density(&options.material).ok_or_else(|| {
        Error::GeminiError(format!("Unknown material: {}", options.material))
    })?;

    // Build analyze options with JSON output
    let mut ai_options = options.analyze_options.clone();
    ai_options.output_format = OutputFormat::Json;

    let files = vec![image_path.to_path_buf()];

    // Step 1: Detect geometry (ensemble, take median of height_m)
    let mut height_m_list: Vec<f64> = Vec::new();

    for run_i in 0..options.ensemble {
        eprintln!(
            "Step 1: Detecting geometry (run {}/{})...",
            run_i + 1,
            options.ensemble
        );

        let raw = match analyze(GEOMETRY_PROMPT, &files, ai_options.clone()) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("  Geometry detection error: {}", e);
                continue;
            }
        };

        let value = match parse_json_response(&raw) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("  JSON parse error: {}", e);
                continue;
            }
        };

        let geo: GeometryResponse = match serde_json::from_value(value) {
            Ok(g) => g,
            Err(e) => {
                eprintln!("  Geometry parse error: {}", e);
                continue;
            }
        };

        let plate_box = match &geo.plate_box {
            Some(pb) if pb.len() == 4 => pb,
            _ => {
                eprintln!("  Plate not detected, skip");
                continue;
            }
        };

        let tg_top = geo.tailgate_top_y.unwrap_or(0.0);
        let tg_bot = geo.tailgate_bottom_y.unwrap_or(0.0);
        let cargo_top = geo.cargo_top_y.unwrap_or(0.0);

        eprintln!(
            "  plate={:?}, tg_top={:.3}, tg_bot={:.3}, cargo_top={:.3}",
            plate_box, tg_top, tg_bot, cargo_top
        );

        if tg_top <= 0.0 || tg_bot <= 0.0 || tg_top >= tg_bot {
            eprintln!("  WARNING: tailgate detection invalid, skip");
            continue;
        }

        // Convert cargoTopY to height in meters above bed floor
        let tg_height_norm = tg_bot - tg_top;
        let m_per_norm = truck.bed_height / tg_height_norm;
        let cargo_height_norm = tg_bot - cargo_top;
        let cargo_height_m = (cargo_height_norm * m_per_norm).clamp(0.0, 0.8);

        eprintln!(
            "  tg_h_norm={:.4}, m/norm={:.3}, cargo_h={:.3}m",
            tg_height_norm, m_per_norm, cargo_height_m
        );
        height_m_list.push(cargo_height_m);
    }

    if height_m_list.is_empty() {
        return Err(Error::GeminiError(
            "Geometry detection failed on all runs".into(),
        ));
    }

    // Take median height
    height_m_list.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let height_m = height_m_list[height_m_list.len() / 2];
    eprintln!(
        "Height estimates: {:?} -> median={:.3}m",
        height_m_list
            .iter()
            .map(|h| format!("{:.3}", h))
            .collect::<Vec<_>>(),
        height_m
    );

    // Step 2: Estimate fill ratios (ensemble, average)
    let mut fill_l_list: Vec<f64> = Vec::new();
    let mut fill_w_list: Vec<f64> = Vec::new();
    let mut packing_list: Vec<f64> = Vec::new();
    let mut last_reasoning = String::new();

    for run_i in 0..options.ensemble {
        eprintln!(
            "Step 2: Estimating fill (run {}/{})...",
            run_i + 1,
            options.ensemble
        );

        let raw = match analyze(FILL_PROMPT, &files, ai_options.clone()) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("  Fill estimation error: {}", e);
                continue;
            }
        };

        let value = match parse_json_response(&raw) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("  JSON parse error: {}", e);
                continue;
            }
        };

        let fill: FillResponse = match serde_json::from_value(value) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("  Fill parse error: {}", e);
                continue;
            }
        };

        let fl = fill.fill_ratio_l.unwrap_or(0.7);
        let fw = fill.fill_ratio_w.unwrap_or(0.8);
        let pd = fill.packing_density.unwrap_or(0.7);

        eprintln!("  L={}, W={}, p={}", fl, fw, pd);

        fill_l_list.push(fl);
        fill_w_list.push(fw);
        packing_list.push(pd);
        if let Some(r) = fill.reasoning {
            last_reasoning = r;
        }
    }

    if fill_l_list.is_empty() {
        return Err(Error::GeminiError("Fill estimation failed on all runs".into()));
    }

    // Average and clamp
    let fill_l = (fill_l_list.iter().sum::<f64>() / fill_l_list.len() as f64).clamp(0.0, 1.0);
    let fill_w = (fill_w_list.iter().sum::<f64>() / fill_w_list.len() as f64).clamp(0.0, 1.0);
    let packing = (packing_list.iter().sum::<f64>() / packing_list.len() as f64).clamp(0.5, 0.9);

    // Step 3: Calculate tonnage
    let volume = truck.bed_length * truck.bed_width * height_m * fill_l * fill_w * SHAPE_FACTOR;
    let tonnage = volume * density * packing;

    Ok(EstimateResult {
        method: "box-overlay".to_string(),
        truck_class: options.truck_class,
        material_type: options.material,
        height_m: round3(height_m),
        fill_ratio_l: round3(fill_l),
        fill_ratio_w: round3(fill_w),
        packing_density: round3(packing),
        estimated_volume_m3: round4(volume),
        estimated_tonnage: round2(tonnage),
        density,
        reasoning: last_reasoning,
    })
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

fn round3(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}

fn round4(v: f64) -> f64 {
    (v * 10000.0).round() / 10000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_truck_spec() {
        let spec = get_truck_spec("4t").unwrap();
        assert!((spec.bed_length - 3.4).abs() < f64::EPSILON);
        assert!((spec.bed_width - 2.06).abs() < f64::EPSILON);
        assert!((spec.bed_height - 0.32).abs() < f64::EPSILON);

        assert!(get_truck_spec("unknown").is_none());
    }

    #[test]
    fn test_get_material_density() {
        assert!((get_material_density("As殻").unwrap() - 2.5).abs() < f64::EPSILON);
        assert!((get_material_density("土砂").unwrap() - 1.8).abs() < f64::EPSILON);
        assert!(get_material_density("unknown").is_none());
    }

    #[test]
    fn test_parse_json_response() {
        let raw = r#"```json
{"plateBox": [0.1, 0.2, 0.3, 0.4], "tailgateTopY": 0.3}
```"#;
        let val = parse_json_response(raw).unwrap();
        assert!(val["plateBox"].is_array());
    }

    #[test]
    fn test_parse_json_response_plain() {
        let raw = r#"{"fillRatioL": 0.7, "fillRatioW": 0.9, "packingDensity": 0.65}"#;
        let val = parse_json_response(raw).unwrap();
        assert!((val["fillRatioL"].as_f64().unwrap() - 0.7).abs() < f64::EPSILON);
    }

    #[test]
    fn test_round_functions() {
        assert!((round2(1.2345) - 1.23).abs() < f64::EPSILON);
        assert!((round3(1.2345) - 1.235).abs() < f64::EPSILON);
        assert!((round4(1.23456) - 1.2346).abs() < f64::EPSILON);
    }
}
