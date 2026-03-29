import { useEffect } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { Photo } from "../types";

interface Props {
  photo: Photo;
  onClose: () => void;
}

export function Lightbox({ photo, onClose }: Props) {
  const src = photo.thumb_path ? convertFileSrc(photo.thumb_path) : null;

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center animate-fade-in"
      style={{ backgroundColor: "rgba(0,0,0,0.92)" }}
      onClick={onClose}
    >
      {/* Close hint */}
      <div className="absolute top-4 right-5 text-xs text-zinc-500 pointer-events-none">
        ESC to close
      </div>

      {src ? (
        <img
          src={src}
          alt={photo.filename}
          onClick={(e) => e.stopPropagation()}
          className="max-w-[92vw] max-h-[92vh] object-contain rounded-xl animate-lightbox-in
                     shadow-2xl shadow-black/60"
        />
      ) : (
        <div className="flex flex-col items-center gap-3 text-zinc-500 animate-lightbox-in">
          <svg
            className="w-16 h-16 opacity-40"
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
          <span className="text-sm">No preview available</span>
        </div>
      )}

      {/* Filename */}
      <div className="absolute bottom-5 left-0 right-0 text-center text-xs text-zinc-500 pointer-events-none">
        {photo.filename}
      </div>
    </div>
  );
}
