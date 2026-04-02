# get-models.ps1
# Downloads and converts the ONNX face models required by Flashback.
# Run once before `npm run tauri dev` or `npm run tauri build`.
#
# Requirements:
#   - Python 3.8+ with pip (for MobileFaceNet conversion only)
#   - Internet connection

$ErrorActionPreference = "Stop"
$modelsDir = "$PSScriptRoot\..\src-tauri\models"

New-Item -ItemType Directory -Force -Path $modelsDir | Out-Null

# ── 1. Ultra-Light face detector (MIT) ────────────────────────────────────────
$detectDest = "$modelsDir\face_detect.onnx"
if (Test-Path $detectDest) {
    Write-Host "face_detect.onnx already present, skipping." -ForegroundColor DarkGray
} else {
    Write-Host "Downloading Ultra-Light face detector (version-RFB-320)..."
    $detectUrl = "https://raw.githubusercontent.com/Linzaer/Ultra-Light-Fast-Generic-Face-Detector-1MB/master/models/onnx/version-RFB-320.onnx"
    Invoke-WebRequest -Uri $detectUrl -OutFile $detectDest
    Write-Host "Saved face_detect.onnx" -ForegroundColor Green
}

# ── 2. MobileFaceNet (Apache 2.0) ─────────────────────────────────────────────
# Converts the pre-trained TensorFlow .pb model from sirius-ai/MobileFaceNet_TF
# to ONNX using tf2onnx. The resulting model has:
#   Input : [1, 112, 112, 3]  NHWC float32, normalised to [-1, 1]
#   Output: [1, 128]          L2-normalised face embedding
$embedDest = "$modelsDir\face_embed.onnx"
if (Test-Path $embedDest) {
    Write-Host "face_embed.onnx already present, skipping." -ForegroundColor DarkGray
    exit 0
}

Write-Host "Setting up MobileFaceNet conversion..."

$tmpDir = "$env:TEMP\mobilefacenet_convert"
New-Item -ItemType Directory -Force -Path $tmpDir | Out-Null

# Download the frozen TF graph
$pbUrl = "https://raw.githubusercontent.com/sirius-ai/MobileFaceNet_TF/master/arch/pretrained_model/MobileFaceNet_9925_9680.pb"
$pbPath = "$tmpDir\MobileFaceNet_9925_9680.pb"
if (-not (Test-Path $pbPath)) {
    Write-Host "Downloading MobileFaceNet TF model..."
    Invoke-WebRequest -Uri $pbUrl -OutFile $pbPath
}

# Install tf2onnx if needed
Write-Host "Installing tf2onnx..."
pip install --quiet tf2onnx tensorflow-cpu

# Convert. The model's native input is NHWC [1, 112, 112, 3].
# tf2onnx will also expose phase_train:0 (a BatchNorm training flag);
# we fold it to False in the post-processing step below.
Write-Host "Converting to ONNX..."
python -m tf2onnx.convert `
    --graphdef "$pbPath" `
    --output "$embedDest" `
    --inputs "input:0[1,112,112,3]" `
    --outputs "embeddings:0" `
    --opset 13

if (-not (Test-Path $embedDest)) {
    Write-Host "Conversion produced no output file." -ForegroundColor Red
    Write-Host "Inspect the model with Netron (https://netron.app) to verify node names." -ForegroundColor Yellow
    exit 1
}

# Fold phase_train=False into the model as a constant initializer so that
# tract sees only one input (input:0) at inference time.
Write-Host "Folding phase_train=False into model..."
python -c "
import onnx, numpy as np
from onnx import numpy_helper
path = r'$embedDest'
model = onnx.load(path)
if 'phase_train:0' not in {i.name for i in model.graph.initializer}:
    model.graph.initializer.append(numpy_helper.from_array(np.array(False), name='phase_train:0'))
for inp in list(model.graph.input):
    if inp.name == 'phase_train:0':
        model.graph.input.remove(inp)
        break
onnx.save(model, path)
print('phase_train folded - model inputs:', [i.name for i in model.graph.input])
"

# Constant-fold the If nodes (TF tf.cond) that tract cannot execute.
# onnxsim uses onnxruntime to evaluate all ops with known-constant inputs,
# collapsing the BatchNorm training branches into plain inference ops.
Write-Host "Simplifying model (constant-folding If nodes)..."
pip install --quiet onnxsim
python -m onnxsim "$embedDest" "$embedDest"

Write-Host "Saved face_embed.onnx" -ForegroundColor Green
Write-Host ""
Write-Host "All models ready. You can now run: npm run tauri dev" -ForegroundColor Cyan
