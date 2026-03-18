import { Photo, SearchResult, Stats, AuthStatus } from "../types";

export function makePhoto(overrides: Partial<Photo> = {}): Photo {
  return {
    id: "photo-1",
    filename: "IMG_001.jpg",
    description: null,
    created_at: "2024-06-15T12:00:00Z",
    width: 4032,
    height: 3024,
    base_url: null,
    mime_type: "image/jpeg",
    is_video: false,
    indexed: false,
    local_path: "/photos/IMG_001.jpg",
    fingerprint: "1718452800_IMG_001.jpg",
    thumb_path: "/app-data/thumbnails/photo-1.jpg",
    ...overrides,
  };
}

export function makeSearchResult(overrides: Partial<SearchResult> = {}): SearchResult {
  return {
    photo: makePhoto(),
    score: 0.87,
    ...overrides,
  };
}

export function makeStats(overrides: Partial<Stats> = {}): Stats {
  return {
    total: 120,
    indexed: 80,
    photos: 100,
    videos: 20,
    ...overrides,
  };
}

export function makeAuthStatus(overrides: Partial<AuthStatus> = {}): AuthStatus {
  return {
    authenticated: false,
    user_name: null,
    ...overrides,
  };
}
