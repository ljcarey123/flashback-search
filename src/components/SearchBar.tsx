import { useState, useRef, useEffect } from "react";

interface Props {
  onSearch: (query: string) => void;
  isSearching: boolean;
}

export function SearchBar({ onSearch, isSearching }: Props) {
  const [query, setQuery] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (query.trim()) onSearch(query.trim());
  };

  return (
    <form onSubmit={handleSubmit} className="relative w-full max-w-2xl mx-auto">
      <div className="relative flex items-center">
        <svg
          className="absolute left-4 w-5 h-5 text-zinc-400 pointer-events-none"
          fill="none"
          stroke="currentColor"
          strokeWidth={2}
          viewBox="0 0 24 24"
        >
          <circle cx="11" cy="11" r="8" />
          <path d="m21 21-4.35-4.35" />
        </svg>

        <input
          ref={inputRef}
          type="text"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder='Search your memories… "me at the beach", "birthday cake 2023"'
          className="w-full pl-12 pr-4 py-3.5 bg-zinc-800/80 backdrop-blur border border-zinc-700
                     rounded-2xl text-zinc-100 placeholder-zinc-500 text-base
                     focus:outline-none focus:border-violet-500 focus:ring-2 focus:ring-violet-500/20
                     transition-all"
        />

        {isSearching && (
          <div className="absolute right-4 w-5 h-5">
            <div className="w-5 h-5 border-2 border-violet-500 border-t-transparent rounded-full animate-spin" />
          </div>
        )}
      </div>
    </form>
  );
}
