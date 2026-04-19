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
    <div className="columns-2 sm:columns-3 lg:columns-4 gap-1.5">
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
            className="relative group cursor-pointer rounded-xl overflow-hidden
                       bg-zinc-900 animate-photo-in mb-1.5 break-inside-avoid
                       hover:ring-2 hover:ring-violet-500/70 hover:shadow-xl hover:shadow-violet-900/30
                       transition-all duration-300"
          >
            {thumbSrc ? (
              <img
                src={thumbSrc}
                alt={photo.filename}
                loading="lazy"
                className="w-full h-auto block group-hover:scale-[1.04] transition-transform duration-500 ease-out"
              />
            ) : (
              <div className="w-full min-h-[120px] flex items-center justify-center">
                <span className="text-zinc-600 text-xs">{photo.filename}</span>
              </div>
            )}

            {/* Gradient overlay with filename on hover */}
            <div className="absolute inset-0 bg-gradient-to-t from-black/70 via-black/20 to-transparent opacity-0 group-hover:opacity-100 transition-opacity duration-300" />
            <div className="absolute bottom-0 left-0 right-0 p-2.5 opacity-0 group-hover:opacity-100 transition-opacity duration-300 flex items-end justify-between gap-1">
              <p className="text-white text-xs font-medium leading-tight line-clamp-1 flex-1 min-w-0">{photo.filename}</p>
              {isSearchResults && score !== null && (
                <span className="text-xs text-violet-400 font-mono font-semibold shrink-0">
                  {(score * 100).toFixed(0)}%
                </span>
              )}
            </div>

            {/* Video badge */}
            {photo.is_video && (
              <div className="absolute top-2 right-2 bg-black/70 backdrop-blur-sm rounded px-1.5 py-0.5 text-xs text-zinc-300 flex items-center gap-1">
                <svg className="w-3 h-3" fill="currentColor" viewBox="0 0 20 20">
                  <path d="M6.3 2.841A1.5 1.5 0 0 0 4 4.11V15.89a1.5 1.5 0 0 0 2.3 1.269l9.344-5.89a1.5 1.5 0 0 0 0-2.538L6.3 2.84Z" />
                </svg>
                Video
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
