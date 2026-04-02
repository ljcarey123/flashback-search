# Flashback

A Windows desktop app for semantic and face-based search over a personal Google Photos library. All indexing and search runs locally — no cloud query service, no ongoing API dependency once photos are imported.

---

## What it does

Photos are indexed by generating a 1536-dimension semantic vector per image using Gemini's multimodal embedding model. Queries are embedded with the same model and ranked by cosine similarity against the local index. The People Engine adds a parallel face-similarity layer: faces are detected and embedded using local ONNX models, and person search finds photos by face likeness rather than semantic content. Both systems compose — you can filter semantically and by person.

---

## Stack

| Layer | Technology |
|---|---|
| Frontend | React 19 · TypeScript · Tailwind CSS v4 |
| Desktop shell | Tauri 2.0 (Rust) |
| Database | SQLite via `rusqlite` (bundled, no native extensions) |
| AI — Embeddings | Gemini Embedding (`gemini-embedding-2-preview`, 1536 dims) |
| AI — Descriptions | Gemini Flash (`gemini-2.5-flash`) |
| Face detection | Ultra-Light RFB-320 via `tract-onnx` |
| Face embeddings | MobileFaceNet via `tract-onnx` |
| Photo import | Google Takeout (bulk) · Google Photos Picker API (incremental) |
| Secret storage | Windows Credential Manager via `keyring` crate |

---

## Technical decisions

**Cosine similarity in pure Rust, no vector extension.**
`sqlite-vec` and similar extensions require a compiled C dependency, which complicates cross-machine builds. Instead, raw `float32` vectors are stored as JSON blobs in SQLite and similarity is computed in Rust on every query. This is O(n) and works well up to ~100k photos on modern hardware. The natural upgrade path when this becomes a bottleneck is HNSW indexing via `usearch` or `sqlite-vec`'s built-in HNSW support — but that adds build complexity for a gain that isn't yet needed.

**ONNX inference via `tract`, no Python or TensorFlow at runtime.**
The face pipeline uses two bundled `.onnx` model files and runs entirely in-process via `tract-onnx`, a pure-Rust ONNX inference library. This keeps the shipped binary self-contained — no Python runtime, no native ML framework. The models are small enough (Ultra-Light ~1.2MB, MobileFaceNet ~4.9MB) that bundling them as Tauri resources is straightforward.

The MobileFaceNet model required a non-trivial conversion pipeline. The original TensorFlow checkpoint uses `tf.cond(phase_train, ...)` inside every BatchNorm layer — a training/inference branch that tf2onnx faithfully converts to ONNX `If` nodes. `tract` does not implement the `If` op. The fix was to fold `phase_train=False` as a constant initializer, then run `onnx-simplifier` to constant-propagate through the graph. This collapsed 104 `If` nodes and all their training-branch ops (ReduceMean, ReduceVariance, Shape, Gather, Cast) into flat inference ops, reducing the model from 5.4MB to 4.9MB and producing a graph tract can execute.

**Dual import paths.**
The Google Photos Library API was deprecated before this project started. The replacement is a hybrid: Google Takeout for bulk one-time imports (no auth, no quota), and the Google Photos Picker API for incremental additions (OAuth, browser-based session). Deduplication uses a `{unix_timestamp}_{filename}` fingerprint that's stable across both sources. Once a thumbnail is on disk, all indexing and search is fully offline.

**Localhost OAuth redirect.**
The Picker auth flow uses a `tokio::TcpListener` on localhost to catch the OAuth `?code=` redirect from the browser — avoiding the deprecated OOB copy-paste flow and keeping the experience closer to a normal web OAuth handshake.

**Concurrent indexing pipeline.**
For each photo, a Gemini Vision description and a Gemini embedding are requested concurrently via `tokio::join!`. The two API calls are independent, so running them in parallel roughly halves indexing time per photo.

**Secure credential storage.**
The Gemini API key and Google OAuth tokens are stored in the Windows Credential Manager via the `keyring` crate — never written to disk as plaintext.

---

## What's implemented

| Area | Detail |
|---|---|
| Import | Takeout bulk import + Picker incremental sync. Deduplication across both. 512px thumbnails generated locally. |
| Semantic indexing | Gemini Vision description + embedding per photo, concurrently. Batched with progress tracking. Re-index support. |
| Semantic search | Text → embedding → cosine similarity over local index. |
| Save layer | Download full-resolution originals to `Pictures\Flashback`. Takeout originals referenced in-place. |
| People engine | Batch face detection (Ultra-Light) + embedding (MobileFaceNet) across the full library. Person management: add/delete people, multiple reference examples per person, centroid averaging across examples. Face-similarity search with adjustable threshold. |
| UI | Animated photo grid, resizable inspector panel, full-screen lightbox, People page with face bbox overlay for reference selection. |

---

## Next tasks

**ANN indexing for scale**
The current linear scan is fast enough for personal libraries but degrades at scale. The right fix is approximate nearest-neighbour search using HNSW (Hierarchical Navigable Small World graphs). Both `usearch` (Rust bindings) and `sqlite-vec`'s built-in HNSW index are viable options. This applies to both the semantic search index and the face embedding index. Deferring until the linear scan is measurably slow is reasonable; the upgrade path is well-defined.

**Better face models**
Ultra-Light RFB-320 and MobileFaceNet are deliberately lightweight models — fast on CPU, easy to bundle, permissively licensed. The accuracy ceiling is low: small faces, off-angle faces, and occlusion all cause missed detections. Replacements to consider:
- Detection: **YuNet** (OpenCV's built-in detector) or **RetinaFace** — meaningfully better recall, still CPU-viable
- Embedding: **ArcFace ResNet-50** — significantly higher recognition accuracy at the cost of a larger model and slower per-face embedding

**Auto-clustering**
Before a user names anyone, run agglomerative clustering over all face embeddings to surface candidate groups. Present these as unlabelled clusters for the user to name, rather than requiring manual reference photo selection. This scales better for large libraries and removes the need to know in advance which photos show which people.

**Combined search**
The semantic and face systems are currently separate query paths. A combined mode — filter by person *and* semantic query — would require intersecting the two result sets, which is straightforward given both return sets of photo IDs with scores.

**Background indexing**
Face detection and embedding currently run as an explicit manual step. The natural behaviour is to run incrementally on import: detect and embed faces for newly imported photos without user intervention, keeping the index current.

**Packaging and onboarding**
Installer, first-run setup flow, and clear credential guidance. The ONNX model acquisition (`scripts/get-models.ps1`) should be integrated into the build or setup step rather than documented as a manual prerequisite.

**Playwright integration tests**
The existing frontend tests (Vitest + React Testing Library) mock Tauri's `invoke` layer, so they don't cover the Rust↔JS boundary. Playwright with `tauri-driver` can drive the real app via WebDriver. Worth adding for the core flows — import trigger, search, person creation — where bugs at the IPC boundary would only surface at runtime.

---

### Deferred

**Location data**
The Picker API does not expose GPS coordinates. EXIF location is available in Takeout originals and JSON sidecars, but since Picker is the primary import path this isn't consistently available. Deferred until there's a clear use case (map view, location search) and a reliable data source.

---

## Getting started

### Prerequisites

- [Rust](https://rustup.rs/) 1.85+
- [Node.js](https://nodejs.org/) 18+
- [Tauri prerequisites for Windows](https://tauri.app/start/prerequisites/)
- Python 3.8+ with pip (for first-time model setup only)

### First-time model setup

The face models are not committed to the repository. Run once before the first build:

```powershell
.\scripts\get-models.ps1
```

This downloads the Ultra-Light detector and converts MobileFaceNet from TensorFlow to ONNX, including the constant-folding step required for `tract` compatibility.

### Run

```bash
npm install
npm run tauri dev
```

### Credentials

1. **Gemini API key** — from [Google AI Studio](https://aistudio.google.com/). Models: `gemini-embedding-2-preview` and `gemini-2.5-flash`. Free tier is sufficient.

2. **Google Cloud credentials** (optional — Picker import only) — OAuth 2.0 Desktop app client from [Google Cloud Console](https://console.cloud.google.com/). Enable the **Google Photos Picker API**. Scopes required:
   - `https://www.googleapis.com/auth/photospicker.mediaitems.readonly`
   - `https://www.googleapis.com/auth/userinfo.profile`

   > Takeout import requires no credentials. Export at [takeout.google.com](https://takeout.google.com), unzip, and point Flashback at the folder.

Enter credentials in the **Settings** page. Both are stored in Windows Credential Manager.

---

## Testing

### Frontend — 43 tests

```bash
npm test               # run once
npm run test:watch     # watch mode
npm run test:coverage  # with coverage report
```

Vitest + React Testing Library. Tauri's `invoke`, `listen`, and `plugin-dialog` are mocked — no binary required.

### Rust — 39 tests

```bash
npm run rust:test
# or:
cargo test --manifest-path src-tauri/Cargo.toml
```

In-memory SQLite and `mockito` for HTTP responses.

---

## Scripts

```bash
# Dev
npm run tauri dev      # Full app (recommended)
npm run dev            # Vite frontend only

# Quality
npm run check          # typecheck + lint + format check
npm run rust:clippy    # Rust clippy
npm run rust:fmt       # Rust rustfmt

# Build
npm run tauri build    # Production .exe
```

---

## Project structure

```
flashback-search/
├── scripts/
│   └── get-models.ps1                # One-time ONNX model acquisition
├── src/                              # React frontend
│   ├── components/
│   │   ├── FaceSelector.tsx          # Face bbox overlay for reference photo selection
│   │   ├── Inspector.tsx             # Resizable side panel: metadata + download
│   │   ├── Lightbox.tsx              # Full-screen image overlay
│   │   ├── PeoplePage.tsx            # People engine UI: indexing, person management, search
│   │   ├── PhotoGrid.tsx             # Animated photo grid
│   │   ├── SearchBar.tsx             # Natural language search input
│   │   └── SettingsPage.tsx          # Import, auth, and indexing controls
│   ├── App.tsx
│   └── types.ts
└── src-tauri/src/                    # Rust backend
    ├── commands.rs                   # All Tauri command handlers
    ├── db.rs                         # SQLite schema, queries, cosine similarity, face search
    ├── face.rs                       # ONNX face detection + embedding pipeline
    ├── gemini.rs                     # Gemini Embedding + Vision API client
    ├── google.rs                     # Google OAuth + Picker API client
    ├── takeout.rs                    # Takeout folder scanner + sidecar parser
    └── secrets.rs                    # Windows Credential Manager wrapper
```

---

## IDE

VS Code + [Tauri extension](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
