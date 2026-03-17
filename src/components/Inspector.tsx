import { invoke } from "@tauri-apps/api/core";
import { useState } from "react";
import { Photo } from "../types";

interface Props {
  photo: Photo;
  onClose: () => void;
}

export function Inspector({ photo, onClose }: Props) {
  const [downloading, setDownloading] = useState(false);
  const [savedPath, setSavedPath] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const download = async () => {
    setDownloading(true);
    setError(null);
    try {
      const path = await invoke<string>("download_photo", { photoId: photo.id });
      setSavedPath(path);
    } catch (e) {
      setError(String(e));
    } finally {
      setDownloading(false);
    }
  };

  const date = photo.created_at
    ? new Date(photo.created_at).toLocaleDateString("en-US", {
        year: "numeric",
        month: "long",
        day: "numeric",
      })
    : "Unknown date";

  return (
    <aside className="w-80 flex-shrink-0 bg-zinc-900 border-l border-zinc-800 flex flex-col overflow-hidden">
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-3 border-b border-zinc-800">
        <span className="text-sm font-medium text-zinc-300">Inspector</span>
        <button onClick={onClose} className="text-zinc-500 hover:text-zinc-300 transition-colors">
          <svg
            className="w-5 h-5"
            fill="none"
            stroke="currentColor"
            strokeWidth={2}
            viewBox="0 0 24 24"
          >
            <path strokeLinecap="round" strokeLinejoin="round" d="M6 18 18 6M6 6l12 12" />
          </svg>
        </button>
      </div>

      {/* Photo preview */}
      <div className="relative bg-zinc-950">
        {photo.base_url ? (
          <img
            src={`${photo.base_url}=w600`}
            alt={photo.filename}
            className="w-full object-contain max-h-64"
          />
        ) : (
          <div className="w-full h-48 flex items-center justify-center bg-zinc-900">
            <span className="text-zinc-600 text-xs">No preview</span>
          </div>
        )}
        {photo.is_video && (
          <div className="absolute inset-0 flex items-center justify-center">
            <div className="w-12 h-12 bg-black/60 rounded-full flex items-center justify-center">
              <svg className="w-6 h-6 text-white ml-1" fill="currentColor" viewBox="0 0 20 20">
                <path d="M6.3 2.841A1.5 1.5 0 0 0 4 4.11V15.89a1.5 1.5 0 0 0 2.3 1.269l9.344-5.89a1.5 1.5 0 0 0 0-2.538L6.3 2.84Z" />
              </svg>
            </div>
          </div>
        )}
      </div>

      {/* Metadata */}
      <div className="flex-1 overflow-y-auto p-4 space-y-3">
        <div>
          <p className="text-xs text-zinc-500 uppercase tracking-wider mb-1">Filename</p>
          <p className="text-sm text-zinc-200 break-all">{photo.filename}</p>
        </div>

        <div>
          <p className="text-xs text-zinc-500 uppercase tracking-wider mb-1">Date</p>
          <p className="text-sm text-zinc-200">{date}</p>
        </div>

        {photo.width && photo.height && (
          <div>
            <p className="text-xs text-zinc-500 uppercase tracking-wider mb-1">Dimensions</p>
            <p className="text-sm text-zinc-200">
              {photo.width} × {photo.height}
            </p>
          </div>
        )}

        {photo.mime_type && (
          <div>
            <p className="text-xs text-zinc-500 uppercase tracking-wider mb-1">Type</p>
            <p className="text-sm text-zinc-200">{photo.mime_type}</p>
          </div>
        )}

        {photo.description && (
          <div>
            <p className="text-xs text-zinc-500 uppercase tracking-wider mb-1">Description</p>
            <p className="text-sm text-zinc-300 italic">{photo.description}</p>
          </div>
        )}

        <div>
          <p className="text-xs text-zinc-500 uppercase tracking-wider mb-1">Status</p>
          <div className="flex items-center gap-2">
            <div
              className={`w-2 h-2 rounded-full ${photo.indexed ? "bg-emerald-500" : "bg-zinc-600"}`}
            />
            <span className="text-sm text-zinc-300">
              {photo.indexed ? "Indexed" : "Not indexed"}
            </span>
          </div>
        </div>

        {savedPath && (
          <div className="bg-emerald-900/30 border border-emerald-700/50 rounded-lg p-3">
            <p className="text-xs text-emerald-400">Saved to:</p>
            <p className="text-xs text-emerald-300 break-all mt-0.5">{savedPath}</p>
          </div>
        )}

        {error && (
          <div className="bg-red-900/30 border border-red-700/50 rounded-lg p-3">
            <p className="text-xs text-red-400">{error}</p>
          </div>
        )}
      </div>

      {/* Actions */}
      <div className="p-4 border-t border-zinc-800">
        <button
          onClick={download}
          disabled={downloading || photo.is_video}
          className="w-full py-2.5 px-4 bg-violet-600 hover:bg-violet-500 disabled:bg-zinc-700
                     disabled:text-zinc-500 text-white text-sm font-medium rounded-xl
                     transition-colors flex items-center justify-center gap-2"
        >
          {downloading ? (
            <>
              <div className="w-4 h-4 border-2 border-white border-t-transparent rounded-full animate-spin" />
              Downloading…
            </>
          ) : (
            <>
              <svg
                className="w-4 h-4"
                fill="none"
                stroke="currentColor"
                strokeWidth={2}
                viewBox="0 0 24 24"
              >
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  d="M3 16.5v2.25A2.25 2.25 0 0 0 5.25 21h13.5A2.25 2.25 0 0 0 21 18.75V16.5M16.5 12 12 16.5m0 0L7.5 12m4.5 4.5V3"
                />
              </svg>
              Save to Pictures\Flashback
            </>
          )}
        </button>
        {photo.is_video && (
          <p className="text-xs text-zinc-600 text-center mt-2">
            Video downloads not supported in MVP
          </p>
        )}
      </div>
    </aside>
  );
}
