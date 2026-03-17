# Flashback

A Windows desktop app that builds a local, searchable index of your Google Photos library using multimodal AI embeddings.

Search your memories with natural language — *"me at the beach"*, *"birthday cake 2023"* — and get visually relevant results in under 100ms.

---

## Current Status

> **Stage 1 — in progress (unverified with real credentials)**

All code through Stage 5 has been written. The immediate next step is connecting real Google Cloud credentials and verifying the OAuth + photo sync flow end-to-end.

| Stage | Status |
|---|---|
| Stage 1 — Cloud Handshake | Code complete. **Needs real-credential verification.** |
| Stage 2 — Vector Engine | Code complete. Blocked on Stage 1. |
| Stage 3 — People Engine | Planned. DB schema stubbed. No code yet. |
| Stage 4 — Semantic Search | Code complete. Blocked on Stage 2. |
| Stage 5 — Save Layer | Code complete. Blocked on Stage 1. |

---

## Stack

| Layer | Technology |
|---|---|
| Frontend | React 19 + TypeScript + Tailwind CSS v4 |
| Desktop shell | Tauri 2.0 (Rust) |
| Database | SQLite via rusqlite (bundled, no native extensions) |
| AI / Embeddings | Gemini Embedding (`gemini-embedding-exp-03-07`) |
| Cloud source | Google Photos API (read-only OAuth 2.0) |
| Secret storage | Windows Credential Manager via `keyring` crate |

---

## Project Stages

### Stage 1 — Cloud Handshake *(code complete, unverified)*
OAuth 2.0 desktop flow (OOB) to get read access to Google Photos. List and sync photo metadata into a local SQLite database.

See [docs/google-photos-integration.md](docs/google-photos-integration.md) for a full breakdown of auth, sync, deduplication, base URL expiry, and daily limits.

### Stage 2 — Vector Engine *(code complete, unverified)*
Download thumbnails → embed with Gemini → store float32 vectors in SQLite. Incremental batch indexing (20 photos per batch) with progress tracking. Videos are skipped.

### Stage 3 — People Engine *(planned)*
Pick a "hero photo" of a person, crop their face, generate a face embedding, and use it to filter the grid.

### Stage 4 — Semantic Search *(code complete, unverified)*
Embed a text query with Gemini and run cosine-similarity search against the indexed library in pure Rust. Target: results in <100ms.

### Stage 5 — Save Layer *(code complete, unverified)*
Download full-resolution originals from Google and write them to `Pictures\Flashback`.

> **Videos** are displayed (metadata + thumbnail) but not indexed. Embedding is photos-only for the MVP.

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

1. **Google Cloud credentials** — create an OAuth 2.0 "Desktop app" client in [Google Cloud Console](https://console.cloud.google.com/). Enable the **Google Photos Library API**. Required scopes:
   - `https://www.googleapis.com/auth/photoslibrary.readonly`
   - `https://www.googleapis.com/auth/userinfo.profile`

2. **Gemini API key** — get one from [Google AI Studio](https://aistudio.google.com/). Model: `gemini-embedding-exp-03-07` (free tier available, ~1,500 RPM).

Enter both in the **Settings** page. Keys are stored in the **Windows Credential Manager** — never written to disk in plaintext.

### Verifying the DB initialised

Open Settings. The Index Health panel shows the SQLite file path (e.g. `C:\Users\...\AppData\Roaming\com.linus.flashback\flashback.db`) and counts. If it shows `0 Total items`, the migration ran cleanly and the app is ready to sync.

---

## Testing

### Frontend — 42 tests

```bash
npm test               # run once
npm run test:watch     # watch mode
npm run test:coverage  # with coverage report
```

Uses **Vitest** + **React Testing Library**. Tauri's `invoke` and `listen` are mocked — no binary required.

### Rust — 29 tests

```bash
npm run rust:test
# or directly:
cargo test --manifest-path src-tauri/Cargo.toml
```

Uses in-memory SQLite (via `rusqlite`) and `mockito` for HTTP responses. Two keychain integration tests are marked `#[ignore]` and must be run explicitly:

```bash
cargo test -- --ignored
```

### E2E

Planned post-Stage 1 verification. Will use `tauri-driver` + Playwright once the OAuth flow is proven with real credentials.

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
│   └── google-photos-integration.md  # Auth, sync, dedup, rate limits
├── src/                              # React frontend
│   ├── components/
│   │   ├── Inspector.tsx             # Side panel: metadata + download
│   │   ├── Inspector.test.tsx
│   │   ├── PhotoGrid.tsx             # Masonry photo wall
│   │   ├── PhotoGrid.test.tsx
│   │   ├── SearchBar.tsx             # Spotlight-style search input
│   │   ├── SearchBar.test.tsx
│   │   ├── SettingsPage.tsx          # OAuth, sync, indexing controls
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
        ├── gemini.rs                 # Gemini Embedding API client
        ├── google.rs                 # Google OAuth + Photos API client
        ├── secrets.rs                # Windows Credential Manager wrapper
        └── lib.rs                    # App setup + plugin registration
```

---

## IDE Setup

VS Code + [Tauri extension](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
