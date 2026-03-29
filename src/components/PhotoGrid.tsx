import { convertFileSrc } from "@tauri-apps/api/core";
import { Photo, SearchResult } from "../types";

interface Props {
  items: Photo[] | SearchResult[];
  onSelect: (photo: Photo) => void;
  isSearchResults?: boolean;
}

function isSearchResult(item: Photo | SearchResult): item is SearchResult {
  return "score" in item;
}

export function PhotoGrid({ items, onSelect, isSearchResults = false }: Props) {
  if (items.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center h-64 text-zinc-500">
        <svg
          className="w-12 h-12 mb-3 opacity-40"
          fill="none"
          stroke="currentColor"
          strokeWidth={1.5}
          viewBox="0 0 24 24"
        >
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            d="m2.25 15.75 5.159-5.159a2.25 2.25 0 0 1 3.182 0l5.159 5.159m-1.5-1.5 1.409-1.409a2.25 2.25 0 0 1 3.182 0l2.909 2.909M3.75 18h16.5M3.375 4.5h17.25c.621 0 1.125.504 1.125 1.125v9.75c0 .621-.504 1.125-1.125 1.125H3.375A1.125 1.125 0 0 1 2.25 15.375V5.625c0-.621.504-1.125 1.125-1.125Z"
          />
        </svg>
        <p className="text-sm">No photos here yet</p>
      </div>
    );
  }

  return (
    <div
      className="grid gap-1.5"
      style={{ gridTemplateColumns: "repeat(auto-fill, minmax(160px, 1fr))" }}
    >
      {items.map((item, index) => {
        const photo = isSearchResult(item) ? item.photo : item;
        const score = isSearchResult(item) ? item.score : null;
        const thumbSrc = photo.thumb_path ? convertFileSrc(photo.thumb_path) : null;
        // Stagger entrance for first 40 items; rest appear instantly
        const animDelay = index < 40 ? index * 28 : 0;

        return (
          <div
            key={photo.id}
            onClick={() => onSelect(photo)}
            style={{ animationDelay: `${animDelay}ms` }}
            className="relative group cursor-pointer rounded-lg overflow-hidden aspect-square
                       bg-zinc-900 animate-photo-in
                       hover:ring-2 hover:ring-violet-500/80 hover:shadow-lg hover:shadow-violet-900/30
                       transition-[box-shadow,outline]"
          >
            {thumbSrc ? (
              <img
                src={thumbSrc}
                alt={photo.filename}
                loading="lazy"
                className="w-full h-full object-cover group-hover:scale-[1.07] transition-transform duration-500 ease-out"
              />
            ) : (
              <div className="w-full h-full flex items-center justify-center">
                <span className="text-zinc-600 text-xs">{photo.filename}</span>
              </div>
            )}

            {/* Subtle gradient vignette on hover */}
            <div className="absolute inset-0 bg-gradient-to-t from-black/40 via-transparent to-transparent opacity-0 group-hover:opacity-100 transition-opacity duration-300" />

            {/* Video badge */}
            {photo.is_video && (
              <div className="absolute top-2 right-2 bg-black/70 backdrop-blur-sm rounded px-1.5 py-0.5 text-xs text-zinc-300 flex items-center gap-1">
                <svg className="w-3 h-3" fill="currentColor" viewBox="0 0 20 20">
                  <path d="M6.3 2.841A1.5 1.5 0 0 0 4 4.11V15.89a1.5 1.5 0 0 0 2.3 1.269l9.344-5.89a1.5 1.5 0 0 0 0-2.538L6.3 2.84Z" />
                </svg>
                Video
              </div>
            )}

            {/* Similarity score */}
            {isSearchResults && score !== null && (
              <div
                className="absolute bottom-0 left-0 right-0 bg-gradient-to-t from-black/90 to-transparent
                              p-2 opacity-0 group-hover:opacity-100 transition-opacity duration-200"
              >
                <div className="text-xs text-violet-400 font-mono font-semibold">
                  {(score * 100).toFixed(1)}% match
                </div>
              </div>
            )}

            {/* Indexed indicator */}
            {photo.indexed && (
              <div
                className="absolute top-2 left-2 w-2 h-2 rounded-full bg-emerald-500 opacity-0 group-hover:opacity-100 transition-opacity"
                title="Indexed"
              />
            )}
          </div>
        );
      })}
    </div>
  );
}
