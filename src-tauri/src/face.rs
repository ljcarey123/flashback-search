/// Face detection and embedding using bundled ONNX models.
///
/// Detection: Ultra-Light face detector (version-RFB-320)
///   - Input  : [1, 3, 240, 320]  float32, values in [0, 1]
///   - Output 0: `scores`  [1, 4420, 2]  – face confidence per anchor
///   - Output 1: `boxes`   [1, 4420, 4]  – [x1,y1,x2,y2] normalised to [0,1]
///
/// Embedding: MobileFaceNet
///   - Input  : [1, 3, 112, 112]  float32, normalised to [-1, 1]
///   - Output : [1, 128]           L2-normalised face embedding
///
/// Both models are bundled as Tauri resources under `models/`.
use anyhow::{anyhow, Result};
use image::imageops::FilterType;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tract_onnx::prelude::*;

/// Normalised face bounding box (all values 0.0–1.0 relative to image dimensions).
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FaceBbox {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

type OnnxModel = SimplePlan<TypedFact, Box<dyn TypedOp>, Graph<TypedFact, Box<dyn TypedOp>>>;

// ── Model loading ─────────────────────────────────────────────────────────────

pub fn load_detection_model(path: &Path) -> Result<OnnxModel> {
    let model = tract_onnx::onnx()
        .model_for_path(path)
        .map_err(|e| anyhow!("Failed to load face detection model from {}: {e}", path.display()))?
        .with_input_fact(
            0,
            InferenceFact::dt_shape(f32::datum_type(), tvec![1usize, 3, 240, 320]),
        )
        .map_err(|e| anyhow!("Detection model input shape error: {e}"))?
        .into_optimized()
        .map_err(|e| anyhow!("Detection model optimisation error: {e}"))?
        .into_runnable()
        .map_err(|e| anyhow!("Detection model compile error: {e}"))?;
    Ok(model)
}

pub fn load_embedding_model(path: &Path) -> Result<OnnxModel> {
    let model = tract_onnx::onnx()
        .model_for_path(path)
        .map_err(|e| anyhow!("Failed to load face embedding model from {}: {e}", path.display()))?
        .with_input_fact(
            0,
            InferenceFact::dt_shape(f32::datum_type(), tvec![1usize, 112, 112, 3]),
        )
        .map_err(|e| anyhow!("Embedding model input shape error: {e}"))?
        .into_optimized()
        .map_err(|e| anyhow!("Embedding model optimisation error: {e}"))?
        .into_runnable()
        .map_err(|e| anyhow!("Embedding model compile error: {e}"))?;
    Ok(model)
}

// ── Detection ─────────────────────────────────────────────────────────────────

/// Detect faces in a JPEG thumbnail. Returns normalised bounding boxes.
pub fn detect_faces(model: &OnnxModel, image_bytes: &[u8]) -> Result<Vec<FaceBbox>> {
    let img = image::load_from_memory(image_bytes)?;

    // Resize to model input dimensions (width=320, height=240)
    let resized = img.resize_exact(320, 240, FilterType::Triangle);
    let rgb = resized.to_rgb8();

    // Build NCHW float tensor, values in [0, 1]
    const H: usize = 240;
    const W: usize = 320;
    let mut data = vec![0.0_f32; 3 * H * W];
    for (i, pixel) in rgb.pixels().enumerate() {
        data[i] = pixel[0] as f32 / 255.0;
        data[H * W + i] = pixel[1] as f32 / 255.0;
        data[2 * H * W + i] = pixel[2] as f32 / 255.0;
    }
    let input: Tensor =
        tract_ndarray::Array4::from_shape_vec((1, 3, H, W), data)?.into();

    let result = model
        .run(tvec![input.into()])
        .map_err(|e| anyhow!("Detection inference error: {e}"))?;

    // scores: [1, 4420, 2] — class 1 is "face"
    // boxes:  [1, 4420, 4] — [x1, y1, x2, y2] normalised to [0,1]
    let scores = result[0].to_array_view::<f32>()?;
    let boxes = result[1].to_array_view::<f32>()?;

    let confidence_threshold = 0.7_f32;
    let mut candidates: Vec<(f32, f32, f32, f32, f32)> = Vec::new(); // (score, x1, y1, x2, y2)

    let n_anchors = scores.shape()[1];
    for i in 0..n_anchors {
        let score = scores[[0, i, 1]];
        if score > confidence_threshold {
            let x1 = boxes[[0, i, 0]].clamp(0.0, 1.0);
            let y1 = boxes[[0, i, 1]].clamp(0.0, 1.0);
            let x2 = boxes[[0, i, 2]].clamp(0.0, 1.0);
            let y2 = boxes[[0, i, 3]].clamp(0.0, 1.0);
            if x2 > x1 && y2 > y1 {
                candidates.push((score, x1, y1, x2, y2));
            }
        }
    }

    let kept = nms(candidates, 0.45);
    Ok(kept
        .into_iter()
        .map(|(_, x1, y1, x2, y2)| FaceBbox {
            x: x1,
            y: y1,
            w: x2 - x1,
            h: y2 - y1,
        })
        .filter(|f| f.w > 0.02 && f.h > 0.02)
        .collect())
}

// ── Embedding ─────────────────────────────────────────────────────────────────

/// Crop a face region and return a 160×160 JPEG (for storage as face_crop_base64).
pub fn crop_face_bytes(image_bytes: &[u8], bbox: &FaceBbox) -> Result<Vec<u8>> {
    let img = image::load_from_memory(image_bytes)?;
    let iw = img.width();
    let ih = img.height();

    let x = (bbox.x.clamp(0.0, 1.0) * iw as f32) as u32;
    let y = (bbox.y.clamp(0.0, 1.0) * ih as f32) as u32;
    let w = ((bbox.w.clamp(0.0, 1.0)) * iw as f32).max(1.0) as u32;
    let h = ((bbox.h.clamp(0.0, 1.0)) * ih as f32).max(1.0) as u32;
    let w = w.min(iw.saturating_sub(x)).max(1);
    let h = h.min(ih.saturating_sub(y)).max(1);

    let crop = img.crop_imm(x, y, w, h);
    let resized = crop.resize_exact(160, 160, FilterType::Lanczos3);

    let mut out = Vec::new();
    resized.write_to(
        &mut std::io::Cursor::new(&mut out),
        image::ImageFormat::Jpeg,
    )?;
    Ok(out)
}

/// Embed a 160×160 face crop (JPEG bytes) using MobileFaceNet.
pub fn embed_face(model: &OnnxModel, crop_bytes: &[u8]) -> Result<Vec<f32>> {
    let img = image::load_from_memory(crop_bytes)?;
    let resized = img.resize_exact(112, 112, FilterType::Lanczos3);
    let rgb = resized.to_rgb8();

    // Build NHWC tensor, normalised to [-1, 1]: (p - 127.5) / 128.0
    // The MobileFaceNet TF model uses channel-last layout [1, 112, 112, 3].
    const S: usize = 112;
    let mut data = vec![0.0_f32; S * S * 3];
    for (i, pixel) in rgb.pixels().enumerate() {
        data[i * 3]     = (pixel[0] as f32 - 127.5) / 128.0;
        data[i * 3 + 1] = (pixel[1] as f32 - 127.5) / 128.0;
        data[i * 3 + 2] = (pixel[2] as f32 - 127.5) / 128.0;
    }
    let input: Tensor =
        tract_ndarray::Array4::from_shape_vec((1, S, S, 3), data)?.into();

    let result = model
        .run(tvec![input.into()])
        .map_err(|e| anyhow!("Embedding inference error: {e}"))?;

    let output = result[0].to_array_view::<f32>()?;
    Ok(output.iter().cloned().collect())
}

// ── NMS ───────────────────────────────────────────────────────────────────────

/// Non-maximum suppression. Candidates are (score, x1, y1, x2, y2).
fn nms(
    mut candidates: Vec<(f32, f32, f32, f32, f32)>,
    iou_threshold: f32,
) -> Vec<(f32, f32, f32, f32, f32)> {
    candidates.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
    let mut kept: Vec<(f32, f32, f32, f32, f32)> = Vec::new();

    'outer: for c in candidates {
        for k in &kept {
            if iou(c, *k) > iou_threshold {
                continue 'outer;
            }
        }
        kept.push(c);
    }
    kept
}

fn iou(
    a: (f32, f32, f32, f32, f32),
    b: (f32, f32, f32, f32, f32),
) -> f32 {
    let ix1 = a.1.max(b.1);
    let iy1 = a.2.max(b.2);
    let ix2 = a.3.min(b.3);
    let iy2 = a.4.min(b.4);

    let inter = (ix2 - ix1).max(0.0) * (iy2 - iy1).max(0.0);
    let area_a = (a.3 - a.1) * (a.4 - a.2);
    let area_b = (b.3 - b.1) * (b.4 - b.2);
    let union = area_a + area_b - inter;
    if union <= 0.0 { 0.0 } else { inter / union }
}
