# Google Photos Integration

How Flashback imports photos from Google, authenticates, and manages local storage.

---

## Import paths

Flashback has two import paths that feed the same local SQLite database. They can be used independently or together.

### Google Takeout (bulk import)

Google Takeout exports your entire library as a folder tree:

```
Takeout/
  Google Photos/
    2023/
      IMG_001.jpg
      IMG_001.jpg.json    ← sidecar with original metadata
    2024/
      ...
```

The Rust `takeout` module (`src-tauri/src/takeout.rs`) walks this folder recursively using `walkdir`, reads each sidecar, and produces a `TakeoutEntry` per media file. No network access or authentication required.

**Sidecar fields used:**

| Field | Purpose |
|---|---|
| `title` | Original filename (falls back to filesystem name) |
| `photoTakenTime.timestamp` | Original capture time (unix epoch string) |
| `creationTime.timestamp` | Fallback timestamp if `photoTakenTime` is absent |
| `description` | Caption/description (optional) |

If no sidecar exists, the filesystem modification time is used as the fallback timestamp.

**Thumbnail generation:** Takeout originals stay in-place. A 512px JPEG thumbnail is generated at import time using the `image` crate (`resize` — preserves aspect ratio) and stored in `{app_data}/thumbnails/{photo_id}.jpg`.

### Google Photos Picker API (incremental import)

The Picker API lets users select specific photos via a Google-hosted browser UI. Unlike the deprecated Library API, it only accesses photos the user explicitly picks.

**Flow:**

```
1. POST /v2/sessions           → create a picker session, get pickerUri
2. Open pickerUri in browser   → user selects photos
3. GET  /v2/sessions/{id}      → poll until mediaItemsSet = true
4. GET  /v2/mediaItems         → list selected items (paginated)
5. Download thumbnail + original for each item
6. DELETE /v2/sessions/{id}    → clean up (best-effort)
```

**URL parameters for downloads:**

| Parameter | Effect |
|---|---|
| `=w512-h512` | Resize to fit within 512×512, preserving aspect ratio (used for thumbnails) |
| `=d` | Full-resolution original download |

The `-c` crop flag (`=w512-h512-c`) was intentionally removed — it produced square-cropped thumbnails that looked wrong in the Inspector panel.

**Downloaded files** are stored in `{app_data}/photos/{filename}` (originals) and `{app_data}/thumbnails/{photo_id}.jpg` (thumbnails).

---

## Authentication (OAuth 2.0 — localhost redirect)

Picker imports require OAuth. Flashback uses a localhost redirect server rather than the deprecated OOB copy-paste flow.

**Flow:**

1. User enters Client ID and Client Secret in Settings (from Google Cloud Console).
2. Flashback binds a TCP listener on a random port (`127.0.0.1:0`) and registers `http://127.0.0.1:{port}` as the redirect URI dynamically.
3. The authorization URL is opened in the user's default browser.
4. Google redirects back to the localhost server with `?code=...`.
5. Flashback parses the code, exchanges it for tokens, saves them to Windows Credential Manager, and the command returns the user's display name.

This is a single blocking `start_auth_flow` command — no two-step code exchange in the UI.

**Required scopes:**

| Scope | Purpose |
|---|---|
| `https://www.googleapis.com/auth/photospicker.mediaitems.readonly` | Access selected photos via Picker API |
| `https://www.googleapis.com/auth/userinfo.profile` | Fetch display name for Settings UI |

**Token storage:**

| Data | Location |
|---|---|
| Access token | Windows Credential Manager |
| Refresh token | Windows Credential Manager |
| Client secret | Windows Credential Manager |
| Client ID | SQLite (non-sensitive, used for re-auth UI) |
| Display name | SQLite (non-sensitive, used for UI) |

---

## Deduplication

Both import paths use a `{unix_timestamp}_{filename}` fingerprint stored with a `UNIQUE` index in SQLite. Inserting a photo with a matching fingerprint is silently skipped.

**Cross-source limitation:** Takeout fingerprints use `photoTakenTime` (EXIF capture time). Picker fingerprints use `createTime` (server upload time). These can differ for photos taken and uploaded at different times, so the same photo imported via both sources may not be deduped. Within a single source, deduplication is reliable.

---

## Indexing pipeline

Indexing is separate from import and is user-triggered via the Settings page. Only photos with a local thumbnail are eligible; videos are skipped.

**Per photo (batches of 20):**

```
1. Read thumbnail JPEG from disk
2. [concurrent]
   a. POST thumbnail bytes to gemini-2.5-flash  → AI description  → store in photos.description
   b. POST thumbnail bytes to gemini-embedding-2-preview (1536 dims) → float32 vector → store in embeddings table
3. Mark photo as indexed=1
4. Emit "index-progress" event to UI
```

The description and embedding calls run concurrently via `tokio::join!`. A failure in either is logged but does not prevent the other from being saved.

**Re-indexing:** The **Re-index All** button clears all embeddings, resets `indexed = 0` for every photo, and starts the first batch. Use this after model changes or to regenerate descriptions.

**Why batches?** The Gemini API has rate limits on the free tier (~1,500 RPM). Batches of 20 allow incremental progress across sessions — if interrupted, the next batch picks up where it left off. Each photo requires two API calls (description + embedding), so a batch of 20 consumes 40 requests.

---

## Videos

Videos are imported with metadata only — no thumbnail is generated and no embedding is created. They appear in the library grid with a video badge but are excluded from semantic search. Full video support would require frame extraction and is out of scope for the current stages.

---

## Local file layout

```
{app_data}/                     e.g. %APPDATA%\com.linus.flashback\
  flashback.db                  SQLite database
  thumbnails/
    {photo_id}.jpg              512px JPEG, one per indexed photo
  photos/
    {original_filename}         Full-res originals (Picker imports only)
```

Takeout originals are referenced in-place via `photos.local_path` — the files stay wherever the user placed the Takeout folder. Moving or deleting the Takeout folder will break the reference (thumbnails remain valid, originals become unavailable for the Save action).
