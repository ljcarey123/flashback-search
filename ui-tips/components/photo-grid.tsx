"use client";

import { PhotoCard, type Photo } from "./photo-card";
import { cn } from "@/lib/utils";

interface PhotoGridProps {
  photos: Photo[];
  layout: "grid" | "masonry";
  selectedPhoto: Photo | null;
  onPhotoSelect: (photo: Photo) => void;
}

export function PhotoGrid({ photos, layout, selectedPhoto, onPhotoSelect }: PhotoGridProps) {
  if (layout === "masonry") {
    // Split photos into columns for masonry layout
    const columns = [[], [], []] as Photo[][];
    photos.forEach((photo, index) => {
      columns[index % 3].push(photo);
    });

    return (
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4">
        {columns.map((column, colIndex) => (
          <div key={colIndex} className="flex flex-col gap-4">
            {column.map((photo) => (
              <PhotoCard
                key={photo.id}
                photo={photo}
                onClick={() => onPhotoSelect(photo)}
                isSelected={selectedPhoto?.id === photo.id}
              />
            ))}
          </div>
        ))}
      </div>
    );
  }

  return (
    <div
      className={cn(
        "grid gap-4",
        "grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4"
      )}
    >
      {photos.map((photo) => (
        <PhotoCard
          key={photo.id}
          photo={{...photo, aspectRatio: "square"}}
          onClick={() => onPhotoSelect(photo)}
          isSelected={selectedPhoto?.id === photo.id}
        />
      ))}
    </div>
  );
}
