"use client";

import { Search, Grid3X3, LayoutGrid, Heart, User } from "lucide-react";
import { useState } from "react";
import { cn } from "@/lib/utils";

interface NavbarProps {
  onLayoutChange: (layout: "grid" | "masonry") => void;
  currentLayout: "grid" | "masonry";
  onSearch: (query: string) => void;
}

export function Navbar({ onLayoutChange, currentLayout, onSearch }: NavbarProps) {
  const [searchQuery, setSearchQuery] = useState("");
  const [activeTab, setActiveTab] = useState("all");

  const tabs = [
    { id: "all", label: "All" },
    { id: "nature", label: "Nature" },
    { id: "architecture", label: "Architecture" },
    { id: "portraits", label: "Portraits" },
    { id: "favorites", label: "Favorites", icon: Heart },
  ];

  return (
    <nav className="fixed top-0 left-0 right-0 z-50 px-4 py-4">
      <div className="glass rounded-2xl max-w-7xl mx-auto">
        <div className="flex items-center justify-between px-6 py-4">
          {/* Logo */}
          <div className="flex items-center gap-3">
            <div className="w-9 h-9 rounded-xl bg-accent/20 flex items-center justify-center">
              <div className="w-5 h-5 rounded-md bg-accent" />
            </div>
            <span className="text-lg font-semibold tracking-tight text-foreground">Gallery</span>
          </div>

          {/* Tabs */}
          <div className="hidden md:flex items-center gap-1 glass-subtle rounded-xl p-1">
            {tabs.map((tab) => (
              <button
                key={tab.id}
                onClick={() => setActiveTab(tab.id)}
                className={cn(
                  "px-4 py-2 rounded-lg text-sm font-medium transition-all duration-300 flex items-center gap-2",
                  activeTab === tab.id
                    ? "bg-foreground/10 text-foreground"
                    : "text-muted-foreground hover:text-foreground hover:bg-foreground/5"
                )}
              >
                {tab.icon && <tab.icon className="w-4 h-4" />}
                {tab.label}
              </button>
            ))}
          </div>

          {/* Actions */}
          <div className="flex items-center gap-3">
            {/* Search */}
            <div className="relative">
              <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted-foreground" />
              <input
                type="text"
                placeholder="Search..."
                value={searchQuery}
                onChange={(e) => {
                  setSearchQuery(e.target.value);
                  onSearch(e.target.value);
                }}
                className="w-48 pl-10 pr-4 py-2 rounded-xl glass-subtle text-sm text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-2 focus:ring-accent/50 transition-all"
              />
            </div>

            {/* Layout Toggle */}
            <div className="flex items-center gap-1 glass-subtle rounded-xl p-1">
              <button
                onClick={() => onLayoutChange("grid")}
                className={cn(
                  "p-2 rounded-lg transition-all duration-300",
                  currentLayout === "grid"
                    ? "bg-foreground/10 text-foreground"
                    : "text-muted-foreground hover:text-foreground"
                )}
              >
                <Grid3X3 className="w-4 h-4" />
              </button>
              <button
                onClick={() => onLayoutChange("masonry")}
                className={cn(
                  "p-2 rounded-lg transition-all duration-300",
                  currentLayout === "masonry"
                    ? "bg-foreground/10 text-foreground"
                    : "text-muted-foreground hover:text-foreground"
                )}
              >
                <LayoutGrid className="w-4 h-4" />
              </button>
            </div>

            {/* User */}
            <button className="p-2 rounded-xl glass-subtle text-muted-foreground hover:text-foreground transition-all">
              <User className="w-5 h-5" />
            </button>
          </div>
        </div>
      </div>
    </nav>
  );
}
