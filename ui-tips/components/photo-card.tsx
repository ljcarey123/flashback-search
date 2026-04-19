"use client";

import { Heart, Maximize2 } from "lucide-react";
import { useState } from "react";
import { cn } from "@/lib/utils";
import Image from "next/image";

export interface Photo {
  id: string;
  src: string;
  title: string;
  author: string;
  likes: number;
  date: string;
  location: string;
  tags: string[];
  aspectRatio: "square" | "portrait" | "landscape";
}

interface PhotoCardProps {
  photo: Photo;
  onClick: () => void;
  isSelected: boolean;
}

export function PhotoCard({ photo, onClick, isSelected }: PhotoCardProps) {
  const [isLiked, setIsLiked] = useState(false);
  const [isHovered, setIsHovered] = useState(false);

  const aspectClasses = {
    square: "aspect-square",
    portrait: "aspect-[3/4]",
    landscape: "aspect-[4/3]",
  };

  return (
    <div
      className={cn(
        "group relative rounded-2xl overflow-hidden cursor-pointer transition-all duration-500",
        aspectClasses[photo.aspectRatio],
        isSelected
          ? "ring-2 ring-accent ring-offset-2 ring-offset-background scale-[0.98]"
          : "hover:scale-[1.02]"
      )}
      onClick={onClick}
      onMouseEnter={() => setIsHovered(true)}
      onMouseLeave={() => setIsHovered(false)}
    >
      {/* Image */}
      <Image
        src={photo.src}
        alt={photo.title}
        fill
        className="object-cover transition-transform duration-700 group-hover:scale-110"
        sizes="(max-width: 640px) 100vw, (max-width: 1024px) 50vw, 33vw"
      />

      {/* Gradient Overlay */}
      <div
        className={cn(
          "absolute inset-0 bg-gradient-to-t from-black/70 via-black/20 to-transparent transition-opacity duration-300",
          isHovered ? "opacity-100" : "opacity-0"
        )}
      />

      {/* Content */}
      <div
        className={cn(
          "absolute inset-0 flex flex-col justify-between p-4 transition-opacity duration-300",
          isHovered ? "opacity-100" : "opacity-0"
        )}
      >
        {/* Top Actions */}
        <div className="flex justify-end">
          <button
            onClick={(e) => {
              e.stopPropagation();
              setIsLiked(!isLiked);
            }}
            className={cn(
              "p-2 rounded-xl glass-subtle transition-all duration-300 hover:scale-110",
              isLiked ? "text-red-500" : "text-white"
            )}
          >
            <Heart className={cn("w-5 h-5", isLiked && "fill-current")} />
          </button>
        </div>

        {/* Bottom Info */}
        <div className="space-y-2">
          <div className="flex items-center justify-between">
            <div>
              <h3 className="text-white font-medium text-sm line-clamp-1">{photo.title}</h3>
              <p className="text-white/70 text-xs">{photo.author}</p>
            </div>
            <button
              onClick={(e) => {
                e.stopPropagation();
                onClick();
              }}
              className="p-2 rounded-xl glass-subtle text-white transition-all duration-300 hover:scale-110"
            >
              <Maximize2 className="w-4 h-4" />
            </button>
          </div>
        </div>
      </div>

      {/* Selection Indicator */}
      {isSelected && (
        <div className="absolute top-3 left-3">
          <div className="w-6 h-6 rounded-full bg-accent flex items-center justify-center">
            <div className="w-2 h-2 rounded-full bg-white" />
          </div>
        </div>
      )}
    </div>
  );
}
