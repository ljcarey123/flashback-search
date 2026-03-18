import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { useEffect, useState } from "react";
import { AuthStatus, ImportSummary, Settings, Stats } from "../types";

interface Props {
  authStatus: AuthStatus;
  onAuthChange: () => void;
}

export function SettingsPage({ authStatus, onAuthChange }: Props) {
  const [hasGeminiKey, setHasGeminiKey] = useState(false);
  const [stats, setStats] = useState<Stats | null>(null);
  const [dbPath, setDbPath] = useState<string | null>(null);
  const [clientId, setClientId] = useState("");
  const [clientSecret, setClientSecret] = useState("");
  const [geminiKey, setGeminiKey] = useState("");

  // Takeout import state
  const [importing, setImporting] = useState(false);
  const [importProgress, setImportProgress] = useState<{
    done: number;
    total: number;
    added: number;
    skipped: number;
  } | null>(null);
  const [importResult, setImportResult] = useState<ImportSummary | null>(null);

  // Picker import state
  const [pickerRunning, setPickerRunning] = useState(false);
  const [pickerStatus, setPickerStatus] = useState<string | null>(null);
  const [pickerResult, setPickerResult] = useState<ImportSummary | null>(null);

  // Indexing state
  const [indexing, setIndexing] = useState(false);

  // Auth state
  const [signingIn, setSigningIn] = useState(false);

  // Debug state
  const [debugging, setDebugging] = useState(false);
  const [debugInfo, setDebugInfo] = useState<{
    token_preview: string;
    tokeninfo_status: number;
    tokeninfo: Record<string, unknown>;
  } | null>(null);

  const [msg, setMsg] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refreshStats = () =>
    invoke<Stats>("get_stats")
      .then(setStats)
      .catch(() => {});

  useEffect(() => {
    invoke<Settings>("load_settings").then((s) => {
      setHasGeminiKey(s.has_gemini_key);
      setClientId(s.client_id ?? "");
    });
    refreshStats();
    invoke<string>("get_db_path")
      .then(setDbPath)
      .catch(() => {});
  }, []);

  // ── Auth ────────────────────────────────────────────────────────────────────

  const startAuthFlow = async () => {
    if (!clientId || !clientSecret) return;
    setSigningIn(true);
    setError(null);
    setMsg(null);
    try {
      const name = await invoke<string>("start_auth_flow", { clientId, clientSecret });
      setMsg(`Signed in as ${name}`);
      setClientSecret("");
      onAuthChange();
    } catch (e) {
      setError(String(e));
    } finally {
      setSigningIn(false);
    }
  };

  const signOut = async () => {
    await invoke("sign_out");
    onAuthChange();
    setMsg(null);
  };

  // ── Takeout import ──────────────────────────────────────────────────────────

  const importTakeout = async () => {
    const folder = await openDialog({
      directory: true,
      multiple: false,
      title: "Select your unzipped Takeout folder",
    });
    if (!folder) return;

    setImporting(true);
    setError(null);
    setImportResult(null);
    setImportProgress(null);

    const unlisten = await listen<{
      done: number;
      total: number;
      added: number;
      skipped: number;
    }>("import-progress", (e) => {
      setImportProgress(e.payload);
    });

    try {
      const result = await invoke<ImportSummary>("import_takeout", {
        folderPath: folder,
      });
      setImportResult(result);
      await refreshStats();
    } catch (e) {
      setError(String(e));
    } finally {
      unlisten();
      setImporting(false);
      setImportProgress(null);
    }
  };

  // ── Picker import ───────────────────────────────────────────────────────────

  const runPickerImport = async () => {
    setPickerRunning(true);
    setError(null);
    setPickerResult(null);
    setPickerStatus("Opening Google Photos Picker in your browser…");

    const unlisten = await listen<{
      status: string;
      total?: number;
      done?: number;
      added?: number;
      skipped?: number;
    }>("picker-status", (e) => {
      const p = e.payload;
      if (p.status === "waiting") {
        setPickerStatus("Waiting for you to select photos in the browser…");
      } else if (p.status === "downloading") {
        const n = p.done ?? 0;
        const t = p.total ?? "?";
        setPickerStatus(`Downloading ${n} / ${t} photos…`);
      } else if (p.status === "done") {
        setPickerStatus(null);
      }
    });

    try {
      const result = await invoke<ImportSummary>("run_picker_import");
      setPickerResult(result);
      await refreshStats();
    } catch (e) {
      setError(String(e));
    } finally {
      unlisten();
      setPickerRunning(false);
      setPickerStatus(null);
    }
  };

  // ── Indexing ────────────────────────────────────────────────────────────────

  const indexBatch = async () => {
    setIndexing(true);
    setError(null);
    try {
      const count = await invoke<number>("index_next_batch", { batchSize: 20 });
      setMsg(`Indexed ${count} photos in this batch`);
      await refreshStats();
    } catch (e) {
      setError(String(e));
    } finally {
      setIndexing(false);
    }
  };

  const reindexAll = async () => {
    setIndexing(true);
    setError(null);
    try {
      await invoke("reset_index");
      const count = await invoke<number>("index_next_batch", { batchSize: 20 });
      setMsg(`Re-indexing started — ${count} photos in first batch`);
      await refreshStats();
    } catch (e) {
      setError(String(e));
    } finally {
      setIndexing(false);
    }
  };

  // ── Gemini key ──────────────────────────────────────────────────────────────

  const saveGeminiKey = async () => {
    await invoke("save_settings", { geminiApiKey: geminiKey });
    setHasGeminiKey(true);
    setGeminiKey("");
    setMsg("Gemini key saved securely to OS keychain");
  };

  // ── Debug ───────────────────────────────────────────────────────────────────

  const debugToken = async () => {
    setDebugging(true);
    setError(null);
    setDebugInfo(null);
    try {
      const info = await invoke<{
        token_preview: string;
        tokeninfo_status: number;
        tokeninfo: Record<string, unknown>;
      }>("debug_token");
      setDebugInfo(info);
    } catch (e) {
      setError(String(e));
    } finally {
      setDebugging(false);
    }
  };

  return (
    <div className="max-w-2xl mx-auto p-6 space-y-8">
      <h1 className="text-2xl font-semibold text-zinc-100">Settings</h1>

      {/* Index Health */}
      {stats && (
        <div className="bg-zinc-900 rounded-2xl p-5 border border-zinc-800">
          <h2 className="text-sm font-medium text-zinc-400 uppercase tracking-wider mb-4">
            Index Health
          </h2>
          <div className="grid grid-cols-3 gap-4">
            {[
              { label: "Total items", value: stats.total },
              { label: "Photos indexed", value: `${stats.indexed} / ${stats.photos}` },
              { label: "Videos (skipped)", value: stats.videos },
            ].map(({ label, value }) => (
              <div key={label} className="bg-zinc-800/50 rounded-xl p-3">
                <p className="text-2xl font-bold text-zinc-100">{value}</p>
                <p className="text-xs text-zinc-500 mt-1">{label}</p>
              </div>
            ))}
          </div>
          {stats.total > 0 && (
            <div className="mt-4">
              <div className="flex justify-between text-xs text-zinc-500 mb-1">
                <span>Indexing progress</span>
                <span>{Math.round((stats.indexed / Math.max(stats.photos, 1)) * 100)}%</span>
              </div>
              <div className="h-1.5 bg-zinc-800 rounded-full overflow-hidden">
                <div
                  className="h-full bg-violet-600 rounded-full transition-all"
                  style={{ width: `${(stats.indexed / Math.max(stats.photos, 1)) * 100}%` }}
                />
              </div>
            </div>
          )}
          {dbPath && (
            <p className="mt-3 text-xs text-zinc-600 font-mono truncate" title={dbPath}>
              DB: {dbPath}
            </p>
          )}
        </div>
      )}

      {/* Sync Center */}
      <div className="bg-zinc-900 rounded-2xl p-5 border border-zinc-800 space-y-5">
        <h2 className="text-sm font-medium text-zinc-400 uppercase tracking-wider">Sync Center</h2>

        {/* ── Import Archive (Takeout) ── */}
        <div className="space-y-3">
          <div>
            <p className="text-sm font-medium text-zinc-200">Import Archive</p>
            <p className="text-xs text-zinc-500 mt-0.5">
              One-time bulk import from an unzipped{" "}
              <span className="text-violet-400">Google Takeout</span> folder. No sign-in required.
            </p>
          </div>

          {importing && importProgress ? (
            <div className="space-y-2">
              <div className="flex justify-between text-xs text-zinc-400">
                <span>
                  {importProgress.done} / {importProgress.total} scanned
                </span>
                <span>
                  {importProgress.added} added · {importProgress.skipped} skipped
                </span>
              </div>
              <div className="h-1.5 bg-zinc-800 rounded-full overflow-hidden">
                <div
                  className="h-full bg-violet-600 rounded-full transition-all"
                  style={{
                    width:
                      importProgress.total > 0
                        ? `${(importProgress.done / importProgress.total) * 100}%`
                        : "0%",
                  }}
                />
              </div>
            </div>
          ) : null}

          {importResult && !importing && (
            <div className="text-xs text-emerald-400 bg-emerald-900/20 border border-emerald-700/40 rounded-lg px-3 py-2">
              Import complete — {importResult.added} added, {importResult.skipped} duplicates
              skipped
              {importResult.errors > 0 && `, ${importResult.errors} errors`}
            </div>
          )}

          <button
            onClick={importTakeout}
            disabled={importing}
            className="w-full py-2.5 px-4 bg-violet-600 hover:bg-violet-500 disabled:bg-zinc-700
                       disabled:text-zinc-500 text-white text-sm rounded-xl transition-colors
                       flex items-center justify-center gap-2"
          >
            {importing ? (
              <>
                <div className="w-4 h-4 border-2 border-white border-t-transparent rounded-full animate-spin" />
                Importing…
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
                    d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z"
                  />
                </svg>
                Choose Takeout Folder
              </>
            )}
          </button>
        </div>

        <div className="border-t border-zinc-800" />

        {/* ── Add Recent Photos (Picker) ── */}
        <div className="space-y-3">
          <div>
            <p className="text-sm font-medium text-zinc-200">Add Recent Photos</p>
            <p className="text-xs text-zinc-500 mt-0.5">
              Open the <span className="text-violet-400">Google Photos Picker</span> to hand-pick
              recent photos. Duplicates already in your library are skipped automatically.
            </p>
          </div>

          {!authStatus.authenticated ? (
            <div className="space-y-3">
              <p className="text-xs text-zinc-500">
                Sign in with your Google account to use the Picker. Required scope:{" "}
                <code className="text-violet-400">photospicker.mediaitems.readonly</code>
              </p>
              <input
                type="text"
                placeholder="OAuth 2.0 Client ID"
                value={clientId}
                onChange={(e) => setClientId(e.target.value)}
                className="w-full px-3 py-2 bg-zinc-800 border border-zinc-700 rounded-xl text-sm
                           text-zinc-200 placeholder-zinc-600 focus:outline-none focus:border-violet-500"
              />
              <input
                type="password"
                placeholder="Client Secret"
                value={clientSecret}
                onChange={(e) => setClientSecret(e.target.value)}
                className="w-full px-3 py-2 bg-zinc-800 border border-zinc-700 rounded-xl text-sm
                           text-zinc-200 placeholder-zinc-600 focus:outline-none focus:border-violet-500"
              />
              <button
                onClick={startAuthFlow}
                disabled={signingIn || !clientId || !clientSecret}
                className="w-full py-2 px-4 bg-zinc-700 hover:bg-zinc-600 disabled:bg-zinc-800
                           disabled:text-zinc-600 text-zinc-200 text-sm rounded-xl transition-colors
                           flex items-center justify-center gap-2"
              >
                {signingIn ? (
                  <>
                    <div className="w-4 h-4 border-2 border-zinc-400 border-t-transparent rounded-full animate-spin" />
                    Waiting for browser sign-in…
                  </>
                ) : (
                  "Sign in with Google"
                )}
              </button>
            </div>
          ) : (
            <div className="space-y-3">
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-2">
                  <div className="w-2 h-2 rounded-full bg-emerald-500" />
                  <span className="text-sm text-zinc-300">
                    Signed in as <strong>{authStatus.user_name}</strong>
                  </span>
                </div>
                <button
                  onClick={signOut}
                  className="text-xs text-zinc-500 hover:text-zinc-300 transition-colors"
                >
                  Sign out
                </button>
              </div>

              {pickerStatus && (
                <div className="text-xs text-violet-400 bg-violet-900/20 border border-violet-700/40 rounded-lg px-3 py-2">
                  {pickerStatus}
                </div>
              )}

              {pickerResult && !pickerRunning && (
                <div className="text-xs text-emerald-400 bg-emerald-900/20 border border-emerald-700/40 rounded-lg px-3 py-2">
                  Done — {pickerResult.added} added, {pickerResult.skipped} duplicates skipped
                  {pickerResult.errors > 0 && `, ${pickerResult.errors} errors`}
                </div>
              )}

              <button
                onClick={runPickerImport}
                disabled={pickerRunning}
                className="w-full py-2.5 px-4 bg-violet-600 hover:bg-violet-500 disabled:bg-zinc-700
                           disabled:text-zinc-500 text-white text-sm rounded-xl transition-colors
                           flex items-center justify-center gap-2"
              >
                {pickerRunning ? (
                  <>
                    <div className="w-4 h-4 border-2 border-white border-t-transparent rounded-full animate-spin" />
                    Running…
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
                      <path strokeLinecap="round" strokeLinejoin="round" d="M12 4v16m8-8H4" />
                    </svg>
                    Add Recent Memories
                  </>
                )}
              </button>
            </div>
          )}
        </div>
      </div>

      {/* Gemini Embedding */}
      <div className="bg-zinc-900 rounded-2xl p-5 border border-zinc-800 space-y-4">
        <h2 className="text-sm font-medium text-zinc-400 uppercase tracking-wider">
          Gemini Embedding
        </h2>
        <p className="text-xs text-zinc-500">
          Used for multimodal indexing and semantic search. Get a key from{" "}
          <span className="text-violet-400">Google AI Studio</span>. Stored in the Windows
          Credential Manager — never written to disk in plaintext.
        </p>

        {hasGeminiKey && (
          <div className="flex items-center gap-2 text-sm text-emerald-400">
            <div className="w-2 h-2 rounded-full bg-emerald-500" />
            API key saved in OS keychain
          </div>
        )}

        <input
          type="password"
          placeholder={hasGeminiKey ? "Enter new key to replace…" : "Gemini API key"}
          value={geminiKey}
          onChange={(e) => setGeminiKey(e.target.value)}
          className="w-full px-3 py-2 bg-zinc-800 border border-zinc-700 rounded-xl text-sm
                     text-zinc-200 placeholder-zinc-600 focus:outline-none focus:border-violet-500"
        />
        <div className="flex gap-2">
          <button
            onClick={saveGeminiKey}
            disabled={!geminiKey}
            className="flex-1 py-2 px-4 bg-zinc-700 hover:bg-zinc-600 disabled:bg-zinc-800
                       disabled:text-zinc-600 text-zinc-200 text-sm rounded-xl transition-colors"
          >
            Save Key
          </button>
          <button
            onClick={indexBatch}
            disabled={indexing || !hasGeminiKey}
            className="flex-1 py-2 px-4 bg-violet-600 hover:bg-violet-500 disabled:bg-zinc-700
                       disabled:text-zinc-500 text-white text-sm rounded-xl transition-colors
                       flex items-center justify-center gap-2"
          >
            {indexing ? (
              <>
                <div className="w-4 h-4 border-2 border-white border-t-transparent rounded-full animate-spin" />
                Indexing…
              </>
            ) : (
              "Index Next Batch (20)"
            )}
          </button>
          <button
            onClick={reindexAll}
            disabled={indexing || !hasGeminiKey}
            className="py-2 px-4 bg-zinc-700 hover:bg-zinc-600 disabled:bg-zinc-800
                       disabled:text-zinc-600 text-zinc-200 text-sm rounded-xl transition-colors"
          >
            Re-index All
          </button>
        </div>
      </div>

      {/* Debug (only when authenticated) */}
      {authStatus.authenticated && (
        <div className="bg-zinc-900 rounded-2xl p-5 border border-zinc-800 space-y-4">
          <h2 className="text-sm font-medium text-zinc-400 uppercase tracking-wider">Debug</h2>
          <button
            onClick={debugToken}
            disabled={debugging}
            className="py-2 px-4 bg-zinc-700 hover:bg-zinc-600 disabled:bg-zinc-800
                       disabled:text-zinc-600 text-zinc-200 text-sm rounded-xl transition-colors
                       flex items-center gap-2"
          >
            {debugging ? (
              <div className="w-4 h-4 border-2 border-zinc-400 border-t-transparent rounded-full animate-spin" />
            ) : null}
            Check Token Scopes
          </button>
          {debugInfo && (
            <div className="space-y-2 text-xs font-mono">
              <p className="text-zinc-400">
                Token: <span className="text-zinc-200">{debugInfo.token_preview}</span>{" "}
                <span
                  className={
                    debugInfo.tokeninfo_status === 200 ? "text-emerald-400" : "text-red-400"
                  }
                >
                  (HTTP {debugInfo.tokeninfo_status})
                </span>
              </p>
              {debugInfo.tokeninfo.scope ? (
                <div>
                  <p className="text-zinc-500 mb-1">Scopes:</p>
                  <ul className="space-y-0.5 pl-2">
                    {String(debugInfo.tokeninfo.scope)
                      .split(" ")
                      .map((s) => (
                        <li
                          key={s}
                          className={
                            s.includes("photospicker") ? "text-emerald-400" : "text-zinc-400"
                          }
                        >
                          {s}
                        </li>
                      ))}
                  </ul>
                </div>
              ) : null}
              {debugInfo.tokeninfo.error ? (
                <p className="text-red-400">Error: {String(debugInfo.tokeninfo.error)}</p>
              ) : null}
            </div>
          )}
        </div>
      )}

      {/* Feedback */}
      {msg && (
        <div className="bg-emerald-900/30 border border-emerald-700/50 rounded-xl p-3 text-sm text-emerald-400">
          {msg}
        </div>
      )}
      {error && (
        <div className="bg-red-900/30 border border-red-700/50 rounded-xl p-3 text-sm text-red-400">
          {error}
        </div>
      )}
    </div>
  );
}
