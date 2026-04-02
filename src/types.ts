export interface Photo {
  id: string;
  filename: string;
  description: string | null;
  created_at: string | null;
  width: number | null;
  height: number | null;
  /** Legacy CDN URL — no longer used for display. */
  base_url: string | null;
  mime_type: string | null;
  is_video: boolean;
  indexed: boolean;
  /** Absolute path to the original file on disk. */
  local_path: string | null;
  /** Dedup key: "{unix_timestamp}_{filename}" */
  fingerprint: string | null;
  /** Absolute path to the 512px thumbnail — computed by Rust, never stored in DB. */
  thumb_path: string | null;
}

export interface SearchResult {
  photo: Photo;
  score: number;
}

export interface Stats {
  total: number;
  indexed: number;
  videos: number;
  photos: number;
}

export interface AuthStatus {
  authenticated: boolean;
  user_name: string | null;
}

export interface Settings {
  /** True if a Gemini key is saved in the OS keychain. */
  has_gemini_key: boolean;
  client_id: string | null;
}

export interface ImportSummary {
  added: number;
  skipped: number;
  errors: number;
}

export interface FaceBbox {
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface Person {
  id: string;
  name: string;
  anchor_photo_id: string;
  face_crop_base64: string | null;
}

export interface PersonExample {
  id: string;
  person_id: string;
  face_crop_base64: string | null;
}

export interface FaceStats {
  photos_pending_detection: number;
  faces_detected: number;
  faces_embedded: number;
}

export type AppView = "library" | "search" | "settings" | "people";
