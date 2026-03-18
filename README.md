# Flashback

A Windows desktop app that builds a private, searchable index of your photo library using multimodal AI embeddings.

Search your memories with natural language — *"me at the beach"*, *"birthday cake 2023"* — and get visually relevant results in under 100ms. Everything runs locally; your photos never leave your machine.

---

## Current Status

> **Stage 2 — Vector Engine (complete)**

| Stage | Status |
|---|---|
| Stage 1 — Import & Auth | Complete. |
| Stage 2 — Vector Engine | Complete. Embedding, AI descriptions, and re-indexing all working. |
| Stage 3 — People Engine | Planned. DB schema stubbed. No code yet. |
| Stage 4 — Semantic Search | Code complete. Functional. |
| Stage 5 — Save Layer | Code complete. |

---

## Stack

| Layer | Technology |
|---|---|
| Frontend | React 19 + TypeScript + Tailwind CSS v4 |
| Desktop shell | Tauri 2.0 (Rust) |
| Database | SQLite via rusqlite (bundled, no native extensions) |
| AI / Embeddings | Gemini Embedding (`gemini-embedding-2-preview`, 1536 dims) |
| AI / Descriptions | Gemini Flash (`gemini-2.5-flash`) |
| Photo import | Google Takeout (bulk) + Google Photos Picker API (incremental) |
| Secret storage | Windows Credential Manager via `keyring` crate |

---

## Project Stages

### Stage 1 — Import & Auth *(complete)*

Two import paths, both feeding the same local SQLite database:

- **Google Takeout** — bulk import from an exported archive folder. No auth required. Reads JSON sidecars for original timestamps and titles. Generates 512px JPEG thumbnails locally via the `image` crate.
- **Google Photos Picker API** — incremental import. User selects photos in a browser Picker UI. Downloads thumbnail + full-resolution original. OAuth via a localhost redirect server (no OOB flow).

Deduplication uses a `{unix_timestamp}_{filename}` fingerprint with a UNIQUE index in SQLite. Re-running either import is safe. Cross-source dedup (Takeout vs Picker for the same photo) is best-effort — see [docs/architecture-decisions.md](docs/architecture-decisions.md).

### Stage 2 — Vector Engine *(complete)*

Incremental batch indexing (20 photos per batch), with concurrent AI description and embedding generation per photo:

1. Read thumbnail from disk
2. Call `gemini-2.5-flash` with image bytes → AI description → stored in `photos.description`
3. Call `gemini-embedding-2-preview` with image bytes → 1536-dim float32 vector → stored in `embeddings` table

Both API calls run concurrently via `tokio::join!`. A **Re-index All** button clears the index and restarts. Videos are skipped. Only photos with a local thumbnail are eligible.

### Stage 3 — People Engine *(planned)*

Pick a "hero photo" of a person → crop face → generate face embedding → use as an anchor for filtered search. This is face-similarity search, not semantic search — see [docs/architecture-decisions.md](docs/architecture-decisions.md) for why.

### Stage 4 — Semantic Search *(code complete)*

Embed a text query with `gemini-embedding-2-preview` and run cosine-similarity search against the indexed library in pure Rust. Both query and stored vectors use the same model and dimension, so similarity scores are meaningful across sources. Target: results in <100ms.

### Stage 5 — Save Layer *(code complete)*

Download full-resolution originals from the Picker import to `Pictures\Flashback`. Takeout originals are referenced in-place from their original folder.

---

## Getting Started

### Prerequisites

- [Rust](https://rustup.rs/) 1.85+
- [Node.js](https://nodejs.org/) 18+
- [Tauri prerequisites for Windows](https://tauri.app/start/prerequisites/)

### Setup

```bash
npm install
npm run tauri dev
```

### You'll need

1. **Google Cloud credentials** — create an OAuth 2.0 "Desktop app" client in [Google Cloud Console](https://console.cloud.google.com/). Enable the **Google Photos Picker API**. Required scope:
   - `https://www.googleapis.com/auth/photospicker.mediaitems.readonly`
   - `https://www.googleapis.com/auth/userinfo.profile`

2. **Gemini API key** — get one from [Google AI Studio](https://aistudio.google.com/). Models used: `gemini-embedding-2-preview` and `gemini-2.0-flash` (free tier available).

Enter both in the **Settings** page. Keys are stored in the **Windows Credential Manager** — never written to disk in plaintext.

### Verifying the DB initialised

Open Settings. The Index Health panel shows the SQLite file path and counts. If it shows `0 Total items` the migration ran cleanly and the app is ready to import.

---

## Testing

### Frontend — 43 tests, Rust — 39 tests

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

Uses in-memory SQLite (via `rusqlite`) and `mockito` for HTTP responses. Two keychain integration tests are marked `#[ignore]`:

```bash
cargo test -- --ignored
```

### E2E

Planned post-Stage 2. Will use `tauri-driver` + Playwright.

---

## Scripts

```bash
# Dev
npm run dev            # Vite frontend only (no Tauri commands)
npm run tauri dev      # Full app (recommended)

# Testing
npm test               # Vitest unit + component tests
npm run test:coverage  # With coverage
npm run rust:test      # Rust unit tests

# Quality (run before committing)
npm run check          # typecheck + lint + format check
npm run rust:clippy    # Rust clippy (warnings as errors)
npm run rust:fmt       # Rust rustfmt

# Build
npm run tauri build    # Production .exe
```

---

## Project Structure

```
flashback-search/
├── docs/
│   ├── architecture-decisions.md     # Key design decisions and trade-offs
│   └── google-photos-integration.md  # Auth, Picker API, rate limits
├── src/                              # React frontend
│   ├── components/
│   │   ├── Inspector.tsx             # Side panel: metadata + download
│   │   ├── Inspector.test.tsx
│   │   ├── PhotoGrid.tsx             # Masonry photo wall
│   │   ├── PhotoGrid.test.tsx
│   │   ├── SearchBar.tsx             # Spotlight-style search input
│   │   ├── SearchBar.test.tsx
│   │   ├── SettingsPage.tsx          # Import, auth, indexing controls
│   │   └── SettingsPage.test.tsx
│   ├── test/
│   │   ├── factories.ts              # Test data builders
│   │   └── setup.ts                  # Vitest + Tauri mock setup
│   ├── App.tsx
│   ├── App.test.tsx
│   ├── main.tsx
│   ├── main.css
│   └── types.ts
└── src-tauri/                        # Rust backend
    └── src/
        ├── commands.rs               # Tauri command handlers (all invoke entrypoints)
        ├── db.rs                     # SQLite schema, queries, cosine similarity
        ├── gemini.rs                 # Gemini Embedding + Vision API client
        ├── google.rs                 # Google OAuth + Picker API client
        ├── takeout.rs                # Google Takeout folder scanner
        ├── secrets.rs                # Windows Credential Manager wrapper
        └── lib.rs                    # App setup + plugin registration
```

---

## IDE Setup

VS Code + [Tauri extension](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
