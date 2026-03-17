# Google Photos Integration

How Flashback connects to Google Photos, fetches your library, and builds a local index.

---

## Authentication (OAuth 2.0 — Desktop Flow)

Google Photos requires OAuth 2.0. Flashback uses the **"Out-of-Band" (OOB) desktop flow** because Tauri apps can't host a localhost redirect server reliably across all Windows configurations.

### Flow

1. User enters their **Client ID** and **Client Secret** in Settings (from Google Cloud Console).
2. Flashback constructs an authorization URL and opens it in the user's default browser.
3. Google presents its consent screen — the user grants access.
4. Google shows a **one-time authorization code** on screen. The user copies and pastes it back into Flashback.
5. Flashback exchanges the code for an **access token** + **refresh token** via a POST to `https://oauth2.googleapis.com/token`.
6. Tokens are stored in the **Windows Credential Manager** (never written to disk in plaintext).

### Required scopes

| Scope | Purpose |
|---|---|
| `https://www.googleapis.com/auth/photoslibrary.readonly` | Read photo metadata and download thumbnails/originals |
| `https://www.googleapis.com/auth/userinfo.profile` | Fetch display name for the Settings UI |

### Token lifetime

- Access tokens expire after **1 hour**.
- The refresh token is long-lived (until the user revokes access in their Google Account).
- **Auto-refresh is planned (Stage 2+)**. Currently, if the access token expires mid-session, the next API call will fail with a 401 and the user must re-authenticate.

### What is stored where

| Data | Location |
|---|---|
| Access token | Windows Credential Manager |
| Refresh token | Windows Credential Manager |
| Client secret | Windows Credential Manager |
| Client ID | SQLite (non-sensitive, used for UI) |
| Display name | SQLite (non-sensitive, used for UI) |

---

## Fetching the Library

The Google Photos Library API returns media items (photos + videos) paginated in batches of up to 100.

### Sync process (`sync_library` command)

```
loop:
  GET /v1/mediaItems?pageSize=100[&pageToken=...]
  → upsert each item into the local `photos` table
  → emit "sync-progress" event to the UI
  until nextPageToken is absent
```

Each page returns:
- `id` — stable unique ID (used as primary key in SQLite)
- `filename`
- `mimeType`
- `baseUrl` — a short-lived (~60 min) URL for downloading content
- `mediaMetadata` — creation time, width, height

The sync can fetch a full 10,000-photo library in roughly 100 API pages (~100 requests). There is no hard daily limit on list requests from the Google Photos API under normal usage.

### Avoiding duplication

Photos are stored with `id` as the **SQLite primary key**. The upsert uses:

```sql
INSERT INTO photos (...) VALUES (...)
ON CONFLICT(id) DO UPDATE SET
  base_url = excluded.base_url,
  description = excluded.description
```

This means:
- Re-running sync never creates duplicate rows.
- The `base_url` is refreshed each sync (important — base URLs expire after ~60 minutes).
- The `indexed` flag is **not touched** during sync, so previously indexed photos stay indexed.

---

## Indexing (Embedding Generation)

Indexing is separate from syncing and is done in manual batches via the "Index Next Batch" button.

### Process (per photo)

```
for each unindexed, non-video photo with a base_url:
  1. Download thumbnail: GET {base_url}=w512-h512-c
  2. Send JPEG bytes to Gemini Embedding API → get a float32 vector
  3. Store vector in `embeddings` table, mark photo as indexed=1
  4. Emit "index-progress" event to UI
```

### Why batches?

- The Gemini API has **rate limits** (requests per minute on the free tier).
- Embedding a large library in one go would block the UI for hours.
- Batches of 20 let you run indexing incrementally across multiple sessions.
- Progress is saved after each photo — if interrupted, the next batch picks up where it left off.

### Daily limits

The Gemini Embedding model (`gemini-embedding-exp-03-07`) has the following free-tier limits as of early 2025:

| Limit | Value |
|---|---|
| Requests per minute | ~1,500 |
| Requests per day | No hard daily cap documented for embeddings |

For a 10,000-photo library, expect roughly **10,000 API calls** total (one per photo thumbnail). At 1,500 RPM this takes ~7 minutes if run continuously. Spread across sessions, it adds up to a few days of evening indexing.

### What about new photos?

Re-running sync fetches new photos from Google Photos and adds them to SQLite with `indexed=0`. They will appear in the library grid immediately but won't show up in semantic search until the next indexing batch covers them.

---

## Ensuring the Entire Library Is Indexed

There is no automatic full-library indexing — it is user-triggered. The Settings page shows:

- **Total items** in the local DB
- **Photos indexed / total photos** (videos are excluded)
- A progress bar

To fully index a large library:
1. Run **Sync Library** first — pulls all metadata.
2. Click **Index Next Batch (20)** repeatedly, or build an "Index All" button (planned) that loops until `get_unindexed_photos` returns 0.

The `get_unindexed_photos` query used internally:

```sql
SELECT ... FROM photos
WHERE indexed = 0
  AND is_video = 0
  AND base_url IS NOT NULL
LIMIT ?
```

---

## Videos

Videos are **not indexed** (no embeddings generated). This is intentional for the MVP:

- Video embedding would require processing multiple frames — much higher API cost.
- Videos are still synced (metadata + thumbnail) and displayed in the grid.
- The `is_video` flag is set based on `mimeType.startsWith("video/")`.

---

## Base URL Expiry

Google Photos `baseUrl` values expire after roughly **60 minutes**. Flashback handles this by:

- Refreshing `base_url` on every sync (the `ON CONFLICT DO UPDATE` path).
- Thumbnails in the grid are loaded lazily from `base_url` — if you leave the app open for hours without syncing, thumbnails will 404.
- Downloads (`=d`) also use `base_url` — syncing before downloading ensures fresh URLs.

A future improvement would be to detect 401/403 responses on image load and trigger a re-sync automatically.
