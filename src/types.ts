export interface Photo {
  id: string;
  filename: string;
  description: string | null;
  created_at: string | null;
  width: number | null;
  height: number | null;
  base_url: string | null;
  mime_type: string | null;
  is_video: boolean;
  indexed: boolean;
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
  /** True if a Gemini key is saved in the OS keychain — the key itself is never sent to the frontend. */
  has_gemini_key: boolean;
  client_id: string | null;
}

export type AppView = "library" | "search" | "settings";
