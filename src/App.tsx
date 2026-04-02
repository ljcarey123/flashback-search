import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "./main.css";

import { SearchBar } from "./components/SearchBar";
import { PhotoGrid } from "./components/PhotoGrid";
import { Inspector } from "./components/Inspector";
import { Lightbox } from "./components/Lightbox";
import { PeoplePage } from "./components/PeoplePage";
import { SettingsPage } from "./components/SettingsPage";
import { AppView, AuthStatus, Photo, SearchResult, Stats } from "./types";

export default function App() {
  const [view, setView] = useState<AppView>("library");
  const [photos, setPhotos] = useState<Photo[]>([]);
  const [searchResults, setSearchResults] = useState<SearchResult[] | null>(null);
  const [selectedPhoto, setSelectedPhoto] = useState<Photo | null>(null);
  const [authStatus, setAuthStatus] = useState<AuthStatus>({
    authenticated: false,
    user_name: null,
  });
  const [stats, setStats] = useState<Stats | null>(null);
  const [isSearching, setIsSearching] = useState(false);
  const [zoomedPhoto, setZoomedPhoto] = useState<Photo | null>(null);
  const [sortOrder, setSortOrder] = useState<"desc" | "asc">("desc");

  // Bootstrap
  useEffect(() => {
    refreshAuth();
    loadLibrary();
  }, []);

  // Listen for sync / index events
  useEffect(() => {
    const unlisten1 = listen("sync-progress", (e) => {
      console.log("sync-progress", e.payload);
    });
    const unlisten2 = listen("index-progress", () => {
      // Reload stats incrementally
      invoke<Stats>("get_stats")
        .then(setStats)
        .catch(() => {});
    });
    return () => {
      unlisten1.then((f) => f());
      unlisten2.then((f) => f());
    };
  }, []);

  const refreshAuth = async () => {
    try {
      const status = await invoke<AuthStatus>("get_auth_status");
      setAuthStatus(status);
    } catch {
      /* ignore */
    }
  };

  const loadLibrary = async () => {
    try {
      const p = await invoke<Photo[]>("get_library");
      setPhotos(p);
      const s = await invoke<Stats>("get_stats");
      setStats(s);
    } catch {
      /* ignore */
    }
  };

  const handleSearch = async (query: string) => {
    setIsSearching(true);
    setView("search");
    try {
      const results = await invoke<SearchResult[]>("search", { query, limit: 50 });
      setSearchResults(results);
    } catch (e) {
      console.error(e);
    } finally {
      setIsSearching(false);
    }
  };

  const sortedPhotos = sortOrder === "desc" ? photos : [...photos].reverse();
  const displayItems = view === "search" && searchResults ? searchResults : sortedPhotos;

  return (
    <div className="flex flex-col h-screen overflow-hidden bg-zinc-950">
      {/* Top bar */}
      <header className="flex items-center gap-4 px-5 py-3 border-b border-zinc-800 bg-zinc-950/95 backdrop-blur shrink-0">
        {/* Logo */}
        <div className="flex items-center gap-2 mr-2">
          <div className="w-7 h-7 rounded-lg bg-violet-600 flex items-center justify-center select-none">
            <span className="text-white font-bold italic text-base leading-none">f</span>
          </div>
          <span className="font-semibold text-zinc-100 text-sm tracking-tight">Flashback</span>
        </div>

        {/* Search bar */}
        <div className="flex-1">
          <SearchBar onSearch={handleSearch} isSearching={isSearching} />
        </div>

        {/* Nav */}
        <nav className="flex items-center gap-1 ml-2">
          {(["library", "people", "settings"] as AppView[]).map((v) => (
            <button
              key={v}
              onClick={() => setView(v)}
              className={`px-3 py-1.5 rounded-lg text-sm font-medium capitalize transition-colors
                ${view === v ? "bg-zinc-800 text-zinc-100" : "text-zinc-500 hover:text-zinc-300 hover:bg-zinc-900"}`}
            >
              {v}
            </button>
          ))}
        </nav>

        {/* Stats pill */}
        {stats && stats.total > 0 && (
          <div className="hidden lg:flex items-center gap-1.5 text-xs text-zinc-500 bg-zinc-900 rounded-full px-3 py-1.5">
            <div
              className={`w-1.5 h-1.5 rounded-full ${authStatus.authenticated ? "bg-emerald-500" : "bg-zinc-600"}`}
            />
            {stats.indexed}/{stats.photos} indexed
          </div>
        )}
      </header>

      {/* Main content */}
      <div className="flex flex-1 overflow-hidden">
        <main className="flex-1 min-w-0 overflow-y-auto">
          {view === "settings" ? (
            <SettingsPage
              authStatus={authStatus}
              onAuthChange={() => {
                refreshAuth();
                loadLibrary();
              }}
            />
          ) : view === "people" ? (
            <PeoplePage
              photos={photos}
              onPersonSearch={(results) => {
                setSearchResults(results);
                setView("search");
              }}
              onSelect={setSelectedPhoto}
            />
          ) : (
            <div className="max-w-5xl mx-auto p-6">
              {/* View header */}
              {view === "search" && searchResults !== null && (
                <div className="flex items-center justify-between mb-4">
                  <span className="text-sm text-zinc-400">
                    {searchResults.length} result{searchResults.length !== 1 ? "s" : ""}
                  </span>
                  <button
                    onClick={() => {
                      setView("library");
                      setSearchResults(null);
                    }}
                    className="text-xs text-zinc-500 hover:text-zinc-300 transition-colors"
                  >
                    ← Back to library
                  </button>
                </div>
              )}

              {view === "library" && (
                <div className="flex items-center justify-between mb-4">
                  <span className="text-sm text-zinc-400">{photos.length} items</span>
                  <div className="flex items-center gap-3">
                    <button
                      onClick={() => setSortOrder((o) => (o === "desc" ? "asc" : "desc"))}
                      className="text-xs text-zinc-500 hover:text-zinc-300 transition-colors flex items-center gap-1"
                    >
                      {sortOrder === "desc" ? "Newest first" : "Oldest first"}
                      <svg
                        className={`w-3 h-3 transition-transform ${sortOrder === "asc" ? "rotate-180" : ""}`}
                        fill="none"
                        stroke="currentColor"
                        strokeWidth={2}
                        viewBox="0 0 24 24"
                      >
                        <path strokeLinecap="round" strokeLinejoin="round" d="M19 9l-7 7-7-7" />
                      </svg>
                    </button>
                    {!authStatus.authenticated && (
                      <button
                        onClick={() => setView("settings")}
                        className="text-xs text-violet-400 hover:text-violet-300 transition-colors"
                      >
                        Connect Google Photos →
                      </button>
                    )}
                  </div>
                </div>
              )}

              <PhotoGrid
                items={displayItems}
                onSelect={setSelectedPhoto}
                isSearchResults={view === "search"}
              />
            </div>
          )}
        </main>

        {/* Inspector panel — always present to avoid layout shifts */}
        <Inspector
          photo={selectedPhoto}
          onClose={() => setSelectedPhoto(null)}
          onZoom={setZoomedPhoto}
        />
      </div>

      {/* Lightbox */}
      {zoomedPhoto && <Lightbox photo={zoomedPhoto} onClose={() => setZoomedPhoto(null)} />}
    </div>
  );
}
