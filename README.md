# Flashback

Search your entire photo library with natural language. *"me at the beach"*, *"birthday cake 2023"*, *"the dog on the sofa"* — results appear in under 100ms. Everything runs locally. Your photos never leave your machine.

---

## What it is

Flashback is a Windows desktop app that builds a private, searchable index of your Google Photos library using multimodal AI embeddings. Once indexed, you can search across thousands of photos using free-form language rather than folders, dates, or tags.

It works by generating a 1536-dimension semantic vector for each photo using Gemini's multimodal embedding model. When you search, your text query is embedded with the same model, and cosine similarity ranks the closest matches — returning visually and contextually relevant results instantly from a local SQLite database.

No cloud search service. No photo uploads for querying. No subscription.

---

## Why it's interesting

Most photo apps search by metadata — date, album, filename, maybe an auto-generated tag. Flashback searches by *meaning*. The same model that understands what's in an image also understands what you're asking for, so results reflect semantic intent rather than keyword matching.

A few things make this technically interesting:

- **No native vector extension required.** Rather than pulling in `sqlite-vec` (which requires a compiled C extension), cosine similarity runs in pure Rust against raw float32 vectors stored in SQLite. This keeps the binary self-contained and the build simple.
- **Dual import paths with no Google API dependency for search.** Bulk libraries come in via Google Takeout (no auth, no API quota). Incremental imports use the Google Photos Picker API (OAuth, browser-based selection). Once a photo is imported and its thumbnail is on disk, all indexing and search runs fully offline.
- **Concurrent AI pipeline.** For each photo, a Gemini Vision description and a Gemini embedding are generated concurrently via `tokio::join!`, keeping indexing throughput high without blocking.
- **Secure credential storage.** The Gemini API key and Google OAuth tokens are stored in the Windows Credential Manager — never written to disk in plaintext.

---

## Stack

| Layer | Technology |
|---|---|
| Frontend | React 19 · TypeScript · Tailwind CSS v4 |
| Desktop shell | Tauri 2.0 (Rust) |
| Database | SQLite via `rusqlite` (bundled, no native extensions) |
| AI — Embeddings | Gemini Embedding (`gemini-embedding-2-preview`, 1536 dims) |
| AI — Descriptions | Gemini Flash (`gemini-2.5-flash`) |
| Photo import | Google Takeout (bulk) · Google Photos Picker API (incremental) |
| Secret storage | Windows Credential Manager via `keyring` crate |

---

## Roadmap

### Done

| Stage | What shipped |
|---|---|
| Import & Auth | Google Takeout bulk import + Google Photos Picker API incremental sync. Deduplication across both sources. 512px thumbnails generated locally. |
| Vector Engine | Incremental batch indexing — Gemini Vision description + embedding generated concurrently per photo. Re-index support. |
| Semantic Search | Text query → embedding → cosine similarity against local index. Sub-100ms results. |
| Save Layer | Download full-resolution originals to `Pictures\Flashback`. Takeout originals referenced in-place. |
| UI Polish | Animated photo grid with staggered entrances, resizable inspector panel, full-screen lightbox. |

### Next

**Sort by date**
The library and search results currently return photos in DB insertion order. Sorting by `created_at` descending is a small change with a big impact on usability.

**People Engine**
Associate photos with specific people. The working concept: pick a reference photo of a person, crop the face, generate a face embedding, and use it as an anchor for filtered search — *"find all photos that include this person"*. This is face-similarity search rather than semantic search; the two systems would compose. Planned stack: Google Cloud Vision API for face detection (bounding boxes) + MobileFaceNet via ONNX Runtime in Rust for local face embeddings.

**Packaging & onboarding**
Installer, first-run setup flow, and clear credential guidance for the portfolio release.

---

### Discussion / Deferred

**Location data**
Google Photos Picker API does not expose GPS coordinates. Location is available via EXIF in downloaded originals and via `geoData` fields in Google Takeout JSON sidecars — but since Picker is the preferred import path for this project, location support is deferred until there is a clear use case (e.g. map view, location-based search). Worth revisiting once the People Engine is stable.

---

## Getting Started

### Prerequisites

- [Rust](https://rustup.rs/) 1.85+
- [Node.js](https://nodejs.org/) 18+
- [Tauri prerequisites for Windows](https://tauri.app/start/prerequisites/)

### Run

```bash
npm install
npm run tauri dev
```

### You'll need

1. **Gemini API key** — from [Google AI Studio](https://aistudio.google.com/). Models used: `gemini-embedding-2-preview` and `gemini-2.5-flash`. Free tier is sufficient for personal libraries.

2. **Google Cloud credentials** (optional — only required for Picker import) — create an OAuth 2.0 "Desktop app" client in [Google Cloud Console](https://console.cloud.google.com/). Enable the **Google Photos Picker API**. Required scopes:
   - `https://www.googleapis.com/auth/photospicker.mediaitems.readonly`
   - `https://www.googleapis.com/auth/userinfo.profile`

   > Google Takeout import works without any credentials. Export your library at [takeout.google.com](https://takeout.google.com), unzip, and point Flashback at the folder.

Enter both in the **Settings** page. Keys are stored in the Windows Credential Manager — never written to disk in plaintext.

---

## Testing

### Frontend — 43 tests

```bash
npm test               # run once
npm run test:watch     # watch mode
npm run test:coverage  # with coverage report
```

Uses **Vitest** + **React Testing Library**. Tauri's `invoke`, `listen`, and `plugin-dialog` are mocked — no binary required.

### Rust — 39 tests

```bash
npm run rust:test
# or directly:
cargo test --manifest-path src-tauri/Cargo.toml
```

Uses in-memory SQLite and `mockito` for HTTP responses.

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

## Project Structure

```
flashback-search/
├── src/                              # React frontend
│   ├── components/
│   │   ├── Inspector.tsx             # Resizable side panel: metadata + download + zoom
│   │   ├── Lightbox.tsx              # Full-screen image overlay
│   │   ├── PhotoGrid.tsx             # Animated photo grid
│   │   ├── SearchBar.tsx             # Natural language search input
│   │   └── SettingsPage.tsx          # Import, auth, and indexing controls
│   ├── App.tsx
│   └── types.ts
└── src-tauri/src/                    # Rust backend
    ├── commands.rs                   # All Tauri command handlers
    ├── db.rs                         # SQLite schema, queries, cosine similarity
    ├── gemini.rs                     # Gemini Embedding + Vision API client
    ├── google.rs                     # Google OAuth + Picker API client
    ├── takeout.rs                    # Takeout folder scanner + sidecar parser
    └── secrets.rs                    # Windows Credential Manager wrapper
```

---

## IDE

VS Code + [Tauri extension](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
