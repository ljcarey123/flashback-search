import { invoke } from "@tauri-apps/api/core";
import { useState, useEffect } from "react";
import { AuthStatus, Settings, Stats } from "../types";

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
  const [authCode, setAuthCode] = useState("");
  const [geminiKey, setGeminiKey] = useState("");
  const [authUrl, setAuthUrl] = useState<string | null>(null);
  const [syncing, setSyncing] = useState(false);
  const [indexing, setIndexing] = useState(false);
  const [syncMsg, setSyncMsg] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [debugInfo, setDebugInfo] = useState<{
    token_preview: string;
    tokeninfo_status: number;
    tokeninfo: Record<string, unknown>;
  } | null>(null);
  const [debugging, setDebugging] = useState(false);
  const [photosApiResult, setPhotosApiResult] = useState<{
    status: number;
    body: unknown;
  } | null>(null);
  const [testingApi, setTestingApi] = useState(false);

  useEffect(() => {
    invoke<Settings>("load_settings").then((s) => {
      setHasGeminiKey(s.has_gemini_key);
      setClientId(s.client_id ?? "");
    });
    invoke<Stats>("get_stats")
      .then(setStats)
      .catch(() => {});
    invoke<string>("get_db_path")
      .then(setDbPath)
      .catch(() => {});
  }, []);

  const startAuth = async () => {
    setError(null);
    try {
      const url = await invoke<string>("get_auth_url", { clientId });
      setAuthUrl(url);
      // Open in default browser
      await invoke("plugin:opener|open_url", { url }).catch(() => {});
    } catch (e) {
      setError(String(e));
    }
  };

  const completeAuth = async () => {
    setError(null);
    try {
      const name = await invoke<string>("exchange_auth_code", {
        clientId,
        clientSecret,
        code: authCode,
      });
      setSyncMsg(`Signed in as ${name}`);
      setAuthUrl(null);
      setAuthCode("");
      onAuthChange();
    } catch (e) {
      setError(String(e));
    }
  };

  const signOut = async () => {
    await invoke("sign_out");
    onAuthChange();
    setSyncMsg(null);
  };

  const syncLibrary = async (maxPages?: number) => {
    setSyncing(true);
    setError(null);
    setSyncMsg(null);
    try {
      const count = await invoke<number>("sync_library", {
        maxPages: maxPages ?? 0,
      });
      setSyncMsg(
        maxPages
          ? `Test sync complete — ${count} items fetched (${maxPages} page${maxPages > 1 ? "s" : ""})`
          : `Full sync complete — ${count} items in library`,
      );
      const s = await invoke<Stats>("get_stats");
      setStats(s);
    } catch (e) {
      setError(String(e));
    } finally {
      setSyncing(false);
    }
  };

  const indexBatch = async () => {
    setIndexing(true);
    setError(null);
    try {
      const count = await invoke<number>("index_next_batch", { batchSize: 20 });
      setSyncMsg(`Indexed ${count} photos in this batch`);
      const s = await invoke<Stats>("get_stats");
      setStats(s);
    } catch (e) {
      setError(String(e));
    } finally {
      setIndexing(false);
    }
  };

  const testPhotosApi = async () => {
    setTestingApi(true);
    setError(null);
    setPhotosApiResult(null);
    try {
      const result = await invoke<{ status: number; body: unknown }>("debug_photos_api");
      setPhotosApiResult(result);
    } catch (e) {
      setError(String(e));
    } finally {
      setTestingApi(false);
    }
  };

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

  const saveGeminiKey = async () => {
    await invoke("save_settings", { geminiApiKey: geminiKey });
    setHasGeminiKey(true);
    setGeminiKey("");
    setSyncMsg("Gemini key saved securely to OS keychain");
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

      {/* Google Auth */}
      <div className="bg-zinc-900 rounded-2xl p-5 border border-zinc-800 space-y-4">
        <h2 className="text-sm font-medium text-zinc-400 uppercase tracking-wider">
          Google Photos
        </h2>

        {authStatus.authenticated ? (
          <div className="space-y-3">
            <div className="flex items-center gap-3">
              <div className="w-2 h-2 rounded-full bg-emerald-500" />
              <span className="text-sm text-zinc-200">
                Signed in as <strong>{authStatus.user_name}</strong>
              </span>
            </div>
            <div className="flex gap-2">
              <button
                onClick={() => syncLibrary(1)}
                disabled={syncing}
                className="py-2 px-4 bg-zinc-700 hover:bg-zinc-600 disabled:bg-zinc-800
                           disabled:text-zinc-600 text-zinc-200 text-sm rounded-xl transition-colors
                           flex items-center justify-center gap-2"
                title="Fetch 1 page (~100 photos) for testing"
              >
                {syncing ? (
                  <div className="w-4 h-4 border-2 border-zinc-400 border-t-transparent rounded-full animate-spin" />
                ) : (
                  "Test Sync"
                )}
              </button>
              <button
                onClick={() => syncLibrary()}
                disabled={syncing}
                className="flex-1 py-2 px-4 bg-violet-600 hover:bg-violet-500 disabled:bg-zinc-700
                           disabled:text-zinc-500 text-white text-sm rounded-xl transition-colors
                           flex items-center justify-center gap-2"
              >
                {syncing ? (
                  <>
                    <div className="w-4 h-4 border-2 border-white border-t-transparent rounded-full animate-spin" />
                    Syncing…
                  </>
                ) : (
                  "Sync Full Library"
                )}
              </button>
              <button
                onClick={signOut}
                className="py-2 px-4 bg-zinc-800 hover:bg-zinc-700 text-zinc-300 text-sm rounded-xl transition-colors"
              >
                Sign out
              </button>
            </div>
          </div>
        ) : (
          <div className="space-y-3">
            <p className="text-xs text-zinc-500">
              Enter your OAuth 2.0 credentials from Google Cloud Console. Required scopes:{" "}
              <code className="text-violet-400">photoslibrary.readonly</code>,{" "}
              <code className="text-violet-400">userinfo.profile</code>
            </p>
            <input
              type="text"
              placeholder="Client ID"
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
              onClick={startAuth}
              disabled={!clientId || !clientSecret}
              className="w-full py-2 px-4 bg-violet-600 hover:bg-violet-500 disabled:bg-zinc-700
                         disabled:text-zinc-500 text-white text-sm rounded-xl transition-colors"
            >
              Open Google Sign-In
            </button>

            {authUrl && (
              <div className="space-y-2">
                <p className="text-xs text-zinc-400">
                  A browser window should have opened. After authorizing, paste the code below:
                </p>
                <input
                  type="text"
                  placeholder="Paste authorization code here"
                  value={authCode}
                  onChange={(e) => setAuthCode(e.target.value)}
                  className="w-full px-3 py-2 bg-zinc-800 border border-zinc-700 rounded-xl text-sm
                             text-zinc-200 placeholder-zinc-600 focus:outline-none focus:border-violet-500"
                />
                <button
                  onClick={completeAuth}
                  disabled={!authCode}
                  className="w-full py-2 px-4 bg-emerald-600 hover:bg-emerald-500 disabled:bg-zinc-700
                             disabled:text-zinc-500 text-white text-sm rounded-xl transition-colors"
                >
                  Complete Sign-In
                </button>
              </div>
            )}
          </div>
        )}
      </div>

      {/* Gemini */}
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
            disabled={indexing || !hasGeminiKey || !authStatus.authenticated}
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
        </div>
      </div>

      {/* Debug */}
      {authStatus.authenticated && (
        <div className="bg-zinc-900 rounded-2xl p-5 border border-zinc-800 space-y-4">
          <h2 className="text-sm font-medium text-zinc-400 uppercase tracking-wider">
            Debug
          </h2>
          <div className="flex gap-2">
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
            <button
              onClick={testPhotosApi}
              disabled={testingApi}
              className="py-2 px-4 bg-zinc-700 hover:bg-zinc-600 disabled:bg-zinc-800
                         disabled:text-zinc-600 text-zinc-200 text-sm rounded-xl transition-colors
                         flex items-center gap-2"
            >
              {testingApi ? (
                <div className="w-4 h-4 border-2 border-zinc-400 border-t-transparent rounded-full animate-spin" />
              ) : null}
              Test Photos API
            </button>
          </div>
          {photosApiResult && (
            <div className="space-y-1 text-xs font-mono">
              <p className="text-zinc-400">
                Photos API:{" "}
                <span className={photosApiResult.status === 200 ? "text-emerald-400" : "text-red-400"}>
                  HTTP {photosApiResult.status}
                </span>
              </p>
              <pre className="text-zinc-400 bg-zinc-800 rounded-lg p-3 overflow-x-auto whitespace-pre-wrap break-all">
                {JSON.stringify(photosApiResult.body, null, 2)}
              </pre>
            </div>
          )}
          {debugInfo && (
            <div className="space-y-2 text-xs font-mono">
              <p className="text-zinc-400">
                Token: <span className="text-zinc-200">{debugInfo.token_preview}</span>
                {" "}
                <span className={debugInfo.tokeninfo_status === 200 ? "text-emerald-400" : "text-red-400"}>
                  (HTTP {debugInfo.tokeninfo_status})
                </span>
              </p>
              {debugInfo.tokeninfo.scope ? (
                <div>
                  <p className="text-zinc-500 mb-1">Scopes:</p>
                  <ul className="space-y-0.5 pl-2">
                    {String(debugInfo.tokeninfo.scope).split(" ").map((s) => (
                      <li
                        key={s}
                        className={
                          s.includes("photoslibrary") ? "text-emerald-400" : "text-zinc-400"
                        }
                      >
                        {s}
                      </li>
                    ))}
                  </ul>
                </div>
              ) : null}
              {debugInfo.tokeninfo.email ? (
                <p className="text-zinc-400">
                  Email: <span className="text-zinc-200">{String(debugInfo.tokeninfo.email)}</span>
                </p>
              ) : null}
              {debugInfo.tokeninfo.expires_in ? (
                <p className="text-zinc-400">
                  Expires in: <span className="text-zinc-200">{String(debugInfo.tokeninfo.expires_in)}s</span>
                </p>
              ) : null}
              {debugInfo.tokeninfo.error ? (
                <p className="text-red-400">Error: {String(debugInfo.tokeninfo.error)}</p>
              ) : null}
            </div>
          )}
        </div>
      )}

      {/* Feedback */}
      {syncMsg && (
        <div className="bg-emerald-900/30 border border-emerald-700/50 rounded-xl p-3 text-sm text-emerald-400">
          {syncMsg}
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
