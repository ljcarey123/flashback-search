"use client";

import { X, Heart, Download, Share2, MapPin, Calendar, Tag, User } from "lucide-react";
import { useState } from "react";
import { cn } from "@/lib/utils";
import Image from "next/image";
import type { Photo } from "./photo-card";

interface PhotoDetailProps {
  photo: Photo | null;
  onClose: () => void;
}

export function PhotoDetail({ photo, onClose }: PhotoDetailProps) {
  const [isLiked, setIsLiked] = useState(false);

  if (!photo) return null;

  return (
    <div
      className={cn(
        "fixed right-0 top-0 bottom-0 w-full md:w-[480px] z-40 glass-strong",
        "transform transition-all duration-500 ease-out",
        "flex flex-col"
      )}
    >
      {/* Header */}
      <div className="flex items-center justify-between px-6 py-4 border-b border-border">
        <h2 className="text-lg font-semibold text-foreground">Photo Details</h2>
        <button
          onClick={onClose}
          className="p-2 rounded-xl glass-subtle text-muted-foreground hover:text-foreground transition-all"
        >
          <X className="w-5 h-5" />
        </button>
      </div>

      {/* Image */}
      <div className="relative aspect-[4/3] m-4 rounded-2xl overflow-hidden">
        <Image
          src={photo.src}
          alt={photo.title}
          fill
          className="object-cover"
          sizes="480px"
          priority
        />
      </div>

      {/* Content */}
      <div className="flex-1 overflow-y-auto px-6 pb-6">
        {/* Title & Author */}
        <div className="mb-6">
          <h3 className="text-2xl font-semibold text-foreground mb-2 text-balance">{photo.title}</h3>
          <div className="flex items-center gap-3">
            <div className="w-10 h-10 rounded-full glass-subtle flex items-center justify-center">
              <User className="w-5 h-5 text-muted-foreground" />
            </div>
            <div>
              <p className="text-sm font-medium text-foreground">{photo.author}</p>
              <p className="text-xs text-muted-foreground">Photographer</p>
            </div>
          </div>
        </div>

        {/* Actions */}
        <div className="flex items-center gap-3 mb-8">
          <button
            onClick={() => setIsLiked(!isLiked)}
            className={cn(
              "flex-1 flex items-center justify-center gap-2 py-3 rounded-xl glass-subtle transition-all duration-300",
              isLiked
                ? "bg-red-500/20 text-red-400"
                : "text-foreground hover:bg-foreground/10"
            )}
          >
            <Heart className={cn("w-5 h-5", isLiked && "fill-current")} />
            <span className="text-sm font-medium">{isLiked ? photo.likes + 1 : photo.likes}</span>
          </button>
          <button className="flex-1 flex items-center justify-center gap-2 py-3 rounded-xl glass-subtle text-foreground hover:bg-foreground/10 transition-all duration-300">
            <Download className="w-5 h-5" />
            <span className="text-sm font-medium">Save</span>
          </button>
          <button className="flex-1 flex items-center justify-center gap-2 py-3 rounded-xl glass-subtle text-foreground hover:bg-foreground/10 transition-all duration-300">
            <Share2 className="w-5 h-5" />
            <span className="text-sm font-medium">Share</span>
          </button>
        </div>

        {/* Info Grid */}
        <div className="space-y-4">
          <h4 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">Information</h4>
          
          <div className="grid grid-cols-2 gap-4">
            <div className="glass-subtle rounded-xl p-4">
              <div className="flex items-center gap-2 mb-2">
                <MapPin className="w-4 h-4 text-accent" />
                <span className="text-xs text-muted-foreground">Location</span>
              </div>
              <p className="text-sm font-medium text-foreground">{photo.location}</p>
            </div>
            
            <div className="glass-subtle rounded-xl p-4">
              <div className="flex items-center gap-2 mb-2">
                <Calendar className="w-4 h-4 text-accent" />
                <span className="text-xs text-muted-foreground">Date</span>
              </div>
              <p className="text-sm font-medium text-foreground">{photo.date}</p>
            </div>
          </div>

          {/* Tags */}
          <div className="glass-subtle rounded-xl p-4">
            <div className="flex items-center gap-2 mb-3">
              <Tag className="w-4 h-4 text-accent" />
              <span className="text-xs text-muted-foreground">Tags</span>
            </div>
            <div className="flex flex-wrap gap-2">
              {photo.tags.map((tag) => (
                <span
                  key={tag}
                  className="px-3 py-1.5 rounded-lg bg-foreground/5 text-xs font-medium text-foreground hover:bg-foreground/10 transition-colors cursor-pointer"
                >
                  {tag}
                </span>
              ))}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
