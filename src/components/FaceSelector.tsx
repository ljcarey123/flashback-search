import { convertFileSrc } from "@tauri-apps/api/core";
import { FaceBbox, Photo } from "../types";

interface Props {
  photo: Photo;
  faces: FaceBbox[];
  selectedIndex: number | null;
  onSelect: (index: number, bbox: FaceBbox) => void;
}

export function FaceSelector({ photo, faces, selectedIndex, onSelect }: Props) {
  const thumbSrc = photo.thumb_path ? convertFileSrc(photo.thumb_path) : null;

  return (
    <div className="relative w-full">
      {thumbSrc ? (
        <img
          src={thumbSrc}
          alt={photo.filename}
          className="w-full block rounded-lg"
          draggable={false}
        />
      ) : (
        <div className="w-full aspect-square bg-zinc-800 rounded-lg flex items-center justify-center">
          <span className="text-zinc-500 text-sm">{photo.filename}</span>
        </div>
      )}

      {/* Face bbox overlays — absolutely positioned over the image */}
      {faces.map((bbox, i) => {
        const isSelected = selectedIndex === i;
        return (
          <div
            key={i}
            onClick={() => onSelect(i, bbox)}
            className={`absolute cursor-pointer transition-all duration-150
              ${
                isSelected
                  ? "border-2 border-violet-400 bg-violet-500/25 shadow-[0_0_0_1px_theme(colors.violet.600)]"
                  : "border-2 border-white/70 bg-transparent hover:border-violet-400 hover:bg-violet-500/15"
              }`}
            style={{
              left: `${bbox.x * 100}%`,
              top: `${bbox.y * 100}%`,
              width: `${bbox.w * 100}%`,
              height: `${bbox.h * 100}%`,
            }}
          />
        );
      })}

      {faces.length === 0 && (
        <div className="absolute inset-0 flex items-center justify-center rounded-lg bg-black/40">
          <span className="text-sm text-zinc-300">No faces detected</span>
        </div>
      )}
    </div>
  );
}
