import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { FaceBbox, FaceStats, Person, PersonExample, Photo, SearchResult } from "../types";
import { PhotoGrid } from "./PhotoGrid";
import { FaceSelector } from "./FaceSelector";

type AddStep = "pick-photo" | "pick-face" | "name-input";

interface Props {
  photos: Photo[];
  onPersonSearch: (results: SearchResult[]) => void;
  onSelect: (photo: Photo) => void;
}

export function PeoplePage({ photos, onPersonSearch, onSelect }: Props) {
  const [people, setPeople] = useState<Person[]>([]);
  const [faceStats, setFaceStats] = useState<FaceStats | null>(null);

  // Add-person flow state
  const [addStep, setAddStep] = useState<AddStep | null>(null);
  const [refPhoto, setRefPhoto] = useState<Photo | null>(null);
  const [detectedFaces, setDetectedFaces] = useState<FaceBbox[]>([]);
  const [selectedFaceIndex, setSelectedFaceIndex] = useState<number | null>(null);
  const [pendingName, setPendingName] = useState("");
  const [isDetecting, setIsDetecting] = useState(false);

  // Add-example flow state (for an existing person)
  const [addExampleForPerson, setAddExampleForPerson] = useState<Person | null>(null);
  const [exampleRefPhoto, setExampleRefPhoto] = useState<Photo | null>(null);
  const [exampleFaces, setExampleFaces] = useState<FaceBbox[]>([]);
  const [exampleFaceIndex, setExampleFaceIndex] = useState<number | null>(null);
  const [isAddingExample, setIsAddingExample] = useState(false);

  // Examples panel for an existing person
  const [viewingExamplesFor, setViewingExamplesFor] = useState<Person | null>(null);
  const [examples, setExamples] = useState<PersonExample[]>([]);

  // Indexing progress
  const [detectProgress, setDetectProgress] = useState<{ done: number; total: number } | null>(
    null,
  );
  const [embedProgress, setEmbedProgress] = useState<{ done: number; total: number } | null>(null);
  const [isIndexing, setIsIndexing] = useState(false);

  // Min similarity threshold for person search
  const [minScore, setMinScore] = useState(0.55);

  useEffect(() => {
    loadPeople();
    loadFaceStats();
  }, []);

  useEffect(() => {
    const u1 = listen<{ done: number; total: number }>("face-detect-progress", (e) => {
      setDetectProgress(e.payload);
    });
    const u2 = listen<{ done: number; total: number }>("face-embed-progress", (e) => {
      setEmbedProgress(e.payload);
    });
    return () => {
      u1.then((f) => f());
      u2.then((f) => f());
    };
  }, []);

  const loadPeople = async () => {
    try {
      const list = await invoke<Person[]>("list_people");
      setPeople(list);
    } catch {
      /* ignore */
    }
  };

  const loadFaceStats = async () => {
    try {
      const stats = await invoke<FaceStats>("get_face_stats");
      setFaceStats(stats);
    } catch {
      /* ignore */
    }
  };

  // ── Add-person flow ──────────────────────────────────────────────────────────

  const handlePickPhotoForAdd = async (photo: Photo) => {
    setRefPhoto(photo);
    setIsDetecting(true);
    setAddStep("pick-face");
    try {
      const faces = await invoke<FaceBbox[]>("detect_faces_for_photo", {
        photoId: photo.id,
      });
      setDetectedFaces(faces);
    } catch (e) {
      console.error("Face detection failed:", e);
      setDetectedFaces([]);
    } finally {
      setIsDetecting(false);
    }
  };

  const handleSelectFaceForAdd = (index: number) => {
    setSelectedFaceIndex(index);
    setAddStep("name-input");
  };

  const handleConfirmAddPerson = async () => {
    if (!refPhoto || selectedFaceIndex === null || !pendingName.trim()) return;
    const bbox = detectedFaces[selectedFaceIndex];
    try {
      const person = await invoke<Person>("create_person", {
        name: pendingName.trim(),
        photoId: refPhoto.id,
        bboxJson: JSON.stringify(bbox),
      });
      setPeople((prev) => [...prev, person].sort((a, b) => a.name.localeCompare(b.name)));
      resetAddFlow();
      loadFaceStats();
    } catch (e) {
      console.error("create_person failed:", e);
    }
  };

  const resetAddFlow = () => {
    setAddStep(null);
    setRefPhoto(null);
    setDetectedFaces([]);
    setSelectedFaceIndex(null);
    setPendingName("");
  };

  // ── Add-example flow ─────────────────────────────────────────────────────────

  const handlePickPhotoForExample = async (photo: Photo) => {
    setExampleRefPhoto(photo);
    setIsDetecting(true);
    try {
      const faces = await invoke<FaceBbox[]>("detect_faces_for_photo", {
        photoId: photo.id,
      });
      setExampleFaces(faces);
    } catch {
      setExampleFaces([]);
    } finally {
      setIsDetecting(false);
    }
  };

  const handleConfirmAddExample = async () => {
    if (!addExampleForPerson || !exampleRefPhoto || exampleFaceIndex === null) return;
    setIsAddingExample(true);
    const bbox = exampleFaces[exampleFaceIndex];
    try {
      await invoke("add_person_example", {
        personId: addExampleForPerson.id,
        photoId: exampleRefPhoto.id,
        bboxJson: JSON.stringify(bbox),
      });
      setAddExampleForPerson(null);
      setExampleRefPhoto(null);
      setExampleFaces([]);
      setExampleFaceIndex(null);
      // Refresh examples panel if open
      if (viewingExamplesFor?.id === addExampleForPerson.id) {
        loadExamples(addExampleForPerson.id);
      }
    } catch (e) {
      console.error("add_person_example failed:", e);
    } finally {
      setIsAddingExample(false);
    }
  };

  // ── Examples panel ────────────────────────────────────────────────────────────

  const loadExamples = async (personId: string) => {
    try {
      const list = await invoke<PersonExample[]>("list_person_examples", { personId });
      setExamples(list);
    } catch {
      /* ignore */
    }
  };

  const handleViewExamples = (person: Person) => {
    setViewingExamplesFor(person);
    loadExamples(person.id);
  };

  const handleDeleteExample = async (example: PersonExample) => {
    if (!viewingExamplesFor) return;
    try {
      await invoke("delete_person_example", {
        exampleId: example.id,
        personId: viewingExamplesFor.id,
      });
      setExamples((prev) => prev.filter((e) => e.id !== example.id));
    } catch (e) {
      console.error("delete_person_example failed:", e);
    }
  };

  // ── People actions ────────────────────────────────────────────────────────────

  const handleDeletePerson = async (personId: string) => {
    try {
      await invoke("delete_person", { personId });
      setPeople((prev) => prev.filter((p) => p.id !== personId));
      if (viewingExamplesFor?.id === personId) setViewingExamplesFor(null);
    } catch (e) {
      console.error("delete_person failed:", e);
    }
  };

  const handleSearch = async (person: Person) => {
    try {
      const results = await invoke<SearchResult[]>("search_by_person", {
        personId: person.id,
        limit: 100,
        minScore,
      });
      onPersonSearch(results);
    } catch (e) {
      console.error("search_by_person failed:", e);
    }
  };

  // ── Face indexing ─────────────────────────────────────────────────────────────

  const handleIndexFaces = async () => {
    setIsIndexing(true);
    setDetectProgress(null);
    setEmbedProgress(null);
    try {
      await invoke("detect_faces_batch", { batchSize: 500 });
      await invoke("embed_faces_batch", { batchSize: 500 });
      await loadFaceStats();
    } catch (e) {
      console.error("Face indexing failed:", e);
    } finally {
      setIsIndexing(false);
      setDetectProgress(null);
      setEmbedProgress(null);
    }
  };

  // ── Render helpers ────────────────────────────────────────────────────────────

  const modelsAvailable = faceStats !== null;

  if (addExampleForPerson) {
    return (
      <div className="max-w-2xl mx-auto p-6 space-y-4">
        <div className="flex items-center gap-3 mb-2">
          <button
            onClick={() => {
              setAddExampleForPerson(null);
              setExampleRefPhoto(null);
              setExampleFaces([]);
              setExampleFaceIndex(null);
            }}
            className="text-xs text-zinc-500 hover:text-zinc-300 transition-colors"
          >
            ← Cancel
          </button>
          <h2 className="text-sm font-medium text-zinc-200">
            Add example for {addExampleForPerson.name}
          </h2>
        </div>

        {!exampleRefPhoto ? (
          <>
            <p className="text-xs text-zinc-500">Pick a photo containing this person.</p>
            <PhotoGrid items={photos} onSelect={handlePickPhotoForExample} />
          </>
        ) : (
          <div className="space-y-4">
            {isDetecting ? (
              <div className="text-sm text-zinc-400">Detecting faces…</div>
            ) : (
              <>
                <FaceSelector
                  photo={exampleRefPhoto}
                  faces={exampleFaces}
                  selectedIndex={exampleFaceIndex}
                  onSelect={(i) => setExampleFaceIndex(i)}
                />
                <p className="text-xs text-zinc-500">
                  {exampleFaces.length === 0
                    ? "No faces detected — try a different photo."
                    : "Click the face to add as a new example."}
                </p>
                {exampleFaceIndex !== null && (
                  <button
                    onClick={handleConfirmAddExample}
                    disabled={isAddingExample}
                    className="px-4 py-2 rounded-lg bg-violet-600 hover:bg-violet-500 text-white text-sm font-medium transition-colors disabled:opacity-50"
                  >
                    {isAddingExample ? "Saving…" : "Add example"}
                  </button>
                )}
              </>
            )}
          </div>
        )}
      </div>
    );
  }

  if (addStep) {
    return (
      <div className="max-w-2xl mx-auto p-6 space-y-4">
        <div className="flex items-center gap-3 mb-2">
          <button
            onClick={resetAddFlow}
            className="text-xs text-zinc-500 hover:text-zinc-300 transition-colors"
          >
            ← Cancel
          </button>
          <h2 className="text-sm font-medium text-zinc-200">
            {addStep === "pick-photo" && "Choose a reference photo"}
            {addStep === "pick-face" && "Select the face"}
            {addStep === "name-input" && "Name this person"}
          </h2>
        </div>

        {addStep === "pick-photo" && (
          <>
            <p className="text-xs text-zinc-500">
              Pick a photo that clearly shows this person's face.
            </p>
            <PhotoGrid items={photos} onSelect={handlePickPhotoForAdd} />
          </>
        )}

        {addStep === "pick-face" && refPhoto && (
          <div className="space-y-4">
            {isDetecting ? (
              <div className="text-sm text-zinc-400">Detecting faces…</div>
            ) : (
              <>
                <FaceSelector
                  photo={refPhoto}
                  faces={detectedFaces}
                  selectedIndex={selectedFaceIndex}
                  onSelect={handleSelectFaceForAdd}
                />
                <p className="text-xs text-zinc-500">
                  {detectedFaces.length === 0
                    ? "No faces detected in this photo — try another."
                    : "Click the face you want to identify."}
                </p>
              </>
            )}
          </div>
        )}

        {addStep === "name-input" && refPhoto && selectedFaceIndex !== null && (
          <div className="space-y-4">
            <div className="w-40 mx-auto">
              <FaceSelector
                photo={refPhoto}
                faces={detectedFaces}
                selectedIndex={selectedFaceIndex}
                onSelect={() => {}}
              />
            </div>
            <input
              autoFocus
              type="text"
              value={pendingName}
              onChange={(e) => setPendingName(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && handleConfirmAddPerson()}
              placeholder="Enter name…"
              className="w-full px-3 py-2 rounded-lg bg-zinc-800 border border-zinc-700 text-zinc-100 text-sm placeholder:text-zinc-500 focus:outline-none focus:ring-1 focus:ring-violet-500"
            />
            <button
              onClick={handleConfirmAddPerson}
              disabled={!pendingName.trim()}
              className="w-full px-4 py-2 rounded-lg bg-violet-600 hover:bg-violet-500 text-white text-sm font-medium transition-colors disabled:opacity-50"
            >
              Save person
            </button>
          </div>
        )}
      </div>
    );
  }

  if (viewingExamplesFor) {
    return (
      <div className="max-w-2xl mx-auto p-6 space-y-4">
        <div className="flex items-center justify-between mb-2">
          <div className="flex items-center gap-3">
            <button
              onClick={() => setViewingExamplesFor(null)}
              className="text-xs text-zinc-500 hover:text-zinc-300 transition-colors"
            >
              ← Back
            </button>
            <h2 className="text-sm font-medium text-zinc-200">
              {viewingExamplesFor.name} — {examples.length} example
              {examples.length !== 1 ? "s" : ""}
            </h2>
          </div>
          <button
            onClick={() => setAddExampleForPerson(viewingExamplesFor)}
            className="text-xs text-violet-400 hover:text-violet-300 transition-colors"
          >
            + Add example
          </button>
        </div>
        <p className="text-xs text-zinc-500">
          More examples improve recognition accuracy. The search uses the average of all examples.
        </p>
        <div className="grid grid-cols-4 gap-2">
          {examples.map((ex) => (
            <div key={ex.id} className="relative group">
              {ex.face_crop_base64 ? (
                <img
                  src={`data:image/jpeg;base64,${ex.face_crop_base64}`}
                  alt="Face example"
                  className="w-full aspect-square object-cover rounded-lg"
                />
              ) : (
                <div className="w-full aspect-square bg-zinc-800 rounded-lg" />
              )}
              <button
                onClick={() => handleDeleteExample(ex)}
                className="absolute top-1 right-1 w-5 h-5 rounded-full bg-black/70 text-zinc-300 hover:text-red-400 text-xs items-center justify-center hidden group-hover:flex transition-colors"
              >
                ×
              </button>
            </div>
          ))}
          {examples.length === 0 && (
            <div className="col-span-4 text-sm text-zinc-500">No examples yet.</div>
          )}
        </div>
      </div>
    );
  }

  return (
    <div className="max-w-5xl mx-auto p-6 space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <h1 className="text-sm font-semibold text-zinc-200">People</h1>
        {people.length > 0 && (
          <button
            onClick={() => setAddStep("pick-photo")}
            className="px-3 py-1.5 rounded-lg bg-violet-600 hover:bg-violet-500 text-white text-xs font-medium transition-colors"
          >
            + Add person
          </button>
        )}
      </div>

      {/* Face indexing panel */}
      {modelsAvailable && faceStats && (
        <div className="rounded-xl border border-zinc-800 bg-zinc-900/60 p-4 space-y-3">
          <div className="flex items-center justify-between">
            <div>
              <p className="text-xs font-medium text-zinc-300">Face index</p>
              <p className="text-xs text-zinc-500 mt-0.5">
                {faceStats.faces_detected} faces detected · {faceStats.faces_embedded} embedded
                {faceStats.photos_pending_detection > 0 && (
                  <span className="text-amber-500 ml-1">
                    · {faceStats.photos_pending_detection} photos pending
                  </span>
                )}
              </p>
            </div>
            <button
              onClick={handleIndexFaces}
              disabled={isIndexing || faceStats.photos_pending_detection === 0}
              className="px-3 py-1.5 rounded-lg bg-zinc-700 hover:bg-zinc-600 text-zinc-200 text-xs font-medium transition-colors disabled:opacity-40"
            >
              {isIndexing ? "Indexing…" : "Index faces"}
            </button>
          </div>

          {detectProgress && (
            <div className="space-y-1">
              <div className="flex justify-between text-xs text-zinc-500">
                <span>Detecting</span>
                <span>
                  {detectProgress.done}/{detectProgress.total}
                </span>
              </div>
              <div className="h-1 bg-zinc-800 rounded-full overflow-hidden">
                <div
                  className="h-full bg-violet-600 rounded-full transition-all"
                  style={{ width: `${(detectProgress.done / detectProgress.total) * 100}%` }}
                />
              </div>
            </div>
          )}
          {embedProgress && (
            <div className="space-y-1">
              <div className="flex justify-between text-xs text-zinc-500">
                <span>Embedding</span>
                <span>
                  {embedProgress.done}/{embedProgress.total}
                </span>
              </div>
              <div className="h-1 bg-zinc-800 rounded-full overflow-hidden">
                <div
                  className="h-full bg-emerald-600 rounded-full transition-all"
                  style={{ width: `${(embedProgress.done / embedProgress.total) * 100}%` }}
                />
              </div>
            </div>
          )}
        </div>
      )}

      {/* Search threshold */}
      {people.length > 0 && (
        <div className="flex items-center gap-3">
          <span className="text-xs text-zinc-500 whitespace-nowrap">
            Min similarity: {Math.round(minScore * 100)}%
          </span>
          <input
            type="range"
            min={0.3}
            max={0.9}
            step={0.05}
            value={minScore}
            onChange={(e) => setMinScore(parseFloat(e.target.value))}
            className="w-32 accent-violet-500"
          />
        </div>
      )}

      {/* People list */}
      {people.length === 0 ? (
        <div className="flex flex-col items-center justify-center py-20 text-zinc-500 space-y-3">
          <svg
            className="w-10 h-10 opacity-30"
            fill="none"
            stroke="currentColor"
            strokeWidth={1.5}
            viewBox="0 0 24 24"
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              d="M15.75 6a3.75 3.75 0 1 1-7.5 0 3.75 3.75 0 0 1 7.5 0ZM4.501 20.118a7.5 7.5 0 0 1 14.998 0A17.933 17.933 0 0 1 12 21.75c-2.676 0-5.216-.584-7.499-1.632Z"
            />
          </svg>
          <p className="text-sm">No people yet</p>
          <p className="text-xs text-center max-w-xs">
            First index your photos' faces using the panel above, then add a person by picking a
            reference photo.
          </p>
          <button
            onClick={() => setAddStep("pick-photo")}
            className="px-4 py-2 rounded-lg bg-violet-600 hover:bg-violet-500 text-white text-sm font-medium transition-colors"
          >
            + Add person
          </button>
        </div>
      ) : (
        <div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 gap-4">
          {people.map((person) => (
            <PersonCard
              key={person.id}
              person={person}
              onSearch={() => handleSearch(person)}
              onViewExamples={() => handleViewExamples(person)}
              onDelete={() => handleDeletePerson(person.id)}
              onSelect={onSelect}
              photos={photos}
            />
          ))}
        </div>
      )}
    </div>
  );
}

// ── PersonCard ─────────────────────────────────────────────────────────────────

interface PersonCardProps {
  person: Person;
  onSearch: () => void;
  onViewExamples: () => void;
  onDelete: () => void;
  onSelect: (photo: Photo) => void;
  photos: Photo[];
}

function PersonCard({ person, onSearch, onViewExamples, onDelete }: PersonCardProps) {
  return (
    <div className="rounded-xl border border-zinc-800 bg-zinc-900/60 overflow-hidden group">
      {/* Face crop thumbnail */}
      <div className="aspect-square bg-zinc-800">
        {person.face_crop_base64 ? (
          <img
            src={`data:image/jpeg;base64,${person.face_crop_base64}`}
            alt={person.name}
            className="w-full h-full object-cover"
          />
        ) : (
          <div className="w-full h-full flex items-center justify-center">
            <svg
              className="w-8 h-8 text-zinc-600"
              fill="none"
              stroke="currentColor"
              strokeWidth={1.5}
              viewBox="0 0 24 24"
            >
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                d="M15.75 6a3.75 3.75 0 1 1-7.5 0 3.75 3.75 0 0 1 7.5 0ZM4.501 20.118a7.5 7.5 0 0 1 14.998 0A17.933 17.933 0 0 1 12 21.75c-2.676 0-5.216-.584-7.499-1.632Z"
              />
            </svg>
          </div>
        )}
      </div>

      {/* Name + actions */}
      <div className="p-2.5 space-y-2">
        <p className="text-xs font-medium text-zinc-200 truncate">{person.name}</p>
        <div className="flex gap-1.5">
          <button
            onClick={onSearch}
            className="flex-1 px-2 py-1 rounded-md bg-violet-600 hover:bg-violet-500 text-white text-xs font-medium transition-colors"
          >
            Search
          </button>
          <button
            onClick={onViewExamples}
            title="Manage face examples"
            className="px-2 py-1 rounded-md bg-zinc-700 hover:bg-zinc-600 text-zinc-300 text-xs transition-colors"
          >
            <svg
              className="w-3.5 h-3.5"
              fill="none"
              stroke="currentColor"
              strokeWidth={2}
              viewBox="0 0 24 24"
            >
              <path strokeLinecap="round" strokeLinejoin="round" d="M12 4.5v15m7.5-7.5h-15" />
            </svg>
          </button>
          <button
            onClick={onDelete}
            title="Delete person"
            className="px-2 py-1 rounded-md bg-zinc-700 hover:bg-red-900/60 text-zinc-400 hover:text-red-400 text-xs transition-colors"
          >
            <svg
              className="w-3.5 h-3.5"
              fill="none"
              stroke="currentColor"
              strokeWidth={2}
              viewBox="0 0 24 24"
            >
              <path strokeLinecap="round" strokeLinejoin="round" d="M6 18 18 6M6 6l12 12" />
            </svg>
          </button>
        </div>
      </div>
    </div>
  );
}
