"use client";

import { useState, useMemo } from "react";
import { Navbar } from "@/components/navbar";
import { PhotoGrid } from "@/components/photo-grid";
import { PhotoDetail } from "@/components/photo-detail";
import type { Photo } from "@/components/photo-card";
import { cn } from "@/lib/utils";

const photos: Photo[] = [
  {
    id: "1",
    src: "https://images.unsplash.com/photo-1682687220742-aba13b6e50ba?w=800&q=80",
    title: "Mountain Sunrise",
    author: "Elena Rivers",
    likes: 2847,
    date: "March 15, 2024",
    location: "Swiss Alps",
    tags: ["nature", "mountains", "sunrise", "landscape"],
    aspectRatio: "landscape",
  },
  {
    id: "2",
    src: "https://images.unsplash.com/photo-1506905925346-21bda4d32df4?w=800&q=80",
    title: "Alpine Peaks",
    author: "Marcus Chen",
    likes: 1923,
    date: "February 28, 2024",
    location: "Dolomites, Italy",
    tags: ["mountains", "nature", "dramatic", "clouds"],
    aspectRatio: "portrait",
  },
  {
    id: "3",
    src: "https://images.unsplash.com/photo-1469474968028-56623f02e42e?w=800&q=80",
    title: "Forest Morning",
    author: "Sarah Woods",
    likes: 3156,
    date: "March 2, 2024",
    location: "Pacific Northwest",
    tags: ["forest", "nature", "fog", "trees"],
    aspectRatio: "square",
  },
  {
    id: "4",
    src: "https://images.unsplash.com/photo-1493246507139-91e8fad9978e?w=800&q=80",
    title: "Lake Reflection",
    author: "David Kim",
    likes: 2341,
    date: "March 8, 2024",
    location: "Lake Louise, Canada",
    tags: ["lake", "reflection", "mountains", "calm"],
    aspectRatio: "landscape",
  },
  {
    id: "5",
    src: "https://images.unsplash.com/photo-1518837695005-2083093ee35b?w=800&q=80",
    title: "Ocean Waves",
    author: "Marina Costa",
    likes: 1876,
    date: "March 12, 2024",
    location: "Big Sur, California",
    tags: ["ocean", "waves", "coast", "dramatic"],
    aspectRatio: "portrait",
  },
  {
    id: "6",
    src: "https://images.unsplash.com/photo-1501785888041-af3ef285b470?w=800&q=80",
    title: "Golden Valley",
    author: "Thomas Berg",
    likes: 2567,
    date: "March 5, 2024",
    location: "Iceland",
    tags: ["valley", "golden", "sunset", "landscape"],
    aspectRatio: "landscape",
  },
  {
    id: "7",
    src: "https://images.unsplash.com/photo-1472214103451-9374bd1c798e?w=800&q=80",
    title: "Meadow Dreams",
    author: "Anna Petrova",
    likes: 1654,
    date: "March 18, 2024",
    location: "New Zealand",
    tags: ["meadow", "green", "peaceful", "nature"],
    aspectRatio: "square",
  },
  {
    id: "8",
    src: "https://images.unsplash.com/photo-1433086966358-54859d0ed716?w=800&q=80",
    title: "Waterfall Cascade",
    author: "James Walker",
    likes: 3421,
    date: "March 1, 2024",
    location: "Norway",
    tags: ["waterfall", "nature", "dramatic", "mist"],
    aspectRatio: "portrait",
  },
  {
    id: "9",
    src: "https://images.unsplash.com/photo-1470071459604-3b5ec3a7fe05?w=800&q=80",
    title: "Foggy Hills",
    author: "Sophie Laurent",
    likes: 2089,
    date: "March 10, 2024",
    location: "Scotland",
    tags: ["hills", "fog", "moody", "landscape"],
    aspectRatio: "landscape",
  },
  {
    id: "10",
    src: "https://images.unsplash.com/photo-1426604966848-d7adac402bff?w=800&q=80",
    title: "River Bend",
    author: "Michael Torres",
    likes: 1789,
    date: "March 20, 2024",
    location: "Yosemite, USA",
    tags: ["river", "nature", "forest", "peaceful"],
    aspectRatio: "square",
  },
  {
    id: "11",
    src: "https://images.unsplash.com/photo-1500534314209-a25ddb2bd429?w=800&q=80",
    title: "Desert Dunes",
    author: "Aisha Patel",
    likes: 2234,
    date: "February 25, 2024",
    location: "Sahara Desert",
    tags: ["desert", "dunes", "sand", "minimal"],
    aspectRatio: "landscape",
  },
  {
    id: "12",
    src: "https://images.unsplash.com/photo-1505144808419-1957a94ca61e?w=800&q=80",
    title: "Tropical Sunset",
    author: "Kai Nakamura",
    likes: 2876,
    date: "March 14, 2024",
    location: "Bali, Indonesia",
    tags: ["sunset", "tropical", "beach", "golden"],
    aspectRatio: "portrait",
  },
];

export default function GalleryPage() {
  const [layout, setLayout] = useState<"grid" | "masonry">("masonry");
  const [selectedPhoto, setSelectedPhoto] = useState<Photo | null>(null);
  const [searchQuery, setSearchQuery] = useState("");

  const filteredPhotos = useMemo(() => {
    if (!searchQuery) return photos;
    const query = searchQuery.toLowerCase();
    return photos.filter(
      (photo) =>
        photo.title.toLowerCase().includes(query) ||
        photo.author.toLowerCase().includes(query) ||
        photo.tags.some((tag) => tag.toLowerCase().includes(query)) ||
        photo.location.toLowerCase().includes(query)
    );
  }, [searchQuery]);

  return (
    <div className="min-h-screen bg-background relative overflow-x-hidden">
      {/* Background Gradient Orbs */}
      <div className="fixed inset-0 overflow-hidden pointer-events-none">
        <div className="absolute -top-40 -left-40 w-96 h-96 bg-accent/20 rounded-full blur-3xl" />
        <div className="absolute top-1/2 -right-40 w-80 h-80 bg-accent/10 rounded-full blur-3xl" />
        <div className="absolute bottom-0 left-1/3 w-72 h-72 bg-accent/15 rounded-full blur-3xl" />
      </div>

      <Navbar
        onLayoutChange={setLayout}
        currentLayout={layout}
        onSearch={setSearchQuery}
      />

      {/* Main Content */}
      <main
        className={cn(
          "relative z-10 pt-28 pb-12 px-4 transition-all duration-500",
          selectedPhoto ? "md:pr-[500px]" : ""
        )}
      >
        <div className="max-w-7xl mx-auto">
          {/* Header */}
          <div className="mb-8">
            <h1 className="text-4xl font-bold text-foreground mb-2 text-balance">
              Discover Beautiful Moments
            </h1>
            <p className="text-muted-foreground text-lg">
              {filteredPhotos.length} photos in your collection
            </p>
          </div>

          {/* Photo Grid */}
          <PhotoGrid
            photos={filteredPhotos}
            layout={layout}
            selectedPhoto={selectedPhoto}
            onPhotoSelect={(photo) =>
              setSelectedPhoto(selectedPhoto?.id === photo.id ? null : photo)
            }
          />
        </div>
      </main>

      {/* Photo Detail Panel */}
      {selectedPhoto && (
        <>
          {/* Backdrop for mobile */}
          <div
            className="fixed inset-0 bg-black/50 z-30 md:hidden"
            onClick={() => setSelectedPhoto(null)}
          />
          <PhotoDetail photo={selectedPhoto} onClose={() => setSelectedPhoto(null)} />
        </>
      )}
    </div>
  );
}
