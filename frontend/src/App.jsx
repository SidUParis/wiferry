import { useCallback, useEffect, useRef, useState } from "react";
import {
  fetchAdminQr,
  guestToken,
  isGuest,
  requestJson,
  uploadWithProgress,
} from "./api.js";
import { Icon } from "./icons.jsx";

const POLL_INTERVAL = 2000;

function usePolling(path, admin) {
  const [state, setState] = useState(null);
  const [error, setError] = useState("");
  const refresh = useCallback(async () => {
    try {
      const next = await requestJson(path, {}, admin);
      setState(next);
      setError("");
    } catch (reason) {
      setError(reason.message);
    }
  }, [admin, path]);

  useEffect(() => {
    const initial = window.setTimeout(refresh, 0);
    const timer = window.setInterval(refresh, POLL_INTERVAL);
    return () => {
      window.clearTimeout(initial);
      window.clearInterval(timer);
    };
  }, [refresh]);
  return { state, setState, error, refresh };
}

function formatCountdown(seconds) {
  if (seconds == null) return "No expiry";
  const minutes = Math.max(0, Math.ceil(seconds / 60));
  return `Expires in ${minutes} min`;
}

function formatTotal(files) {
  const bytes = files.reduce((total, file) => total + file.size, 0);
  if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  return `${(bytes / 1024 / 1024 / 1024).toFixed(1)} GB`;
}

function isTailnet(state) {
  return state?.transport === "tailscale";
}

function hostCandidates(state) {
  return (state.hostCandidates || []).map((candidate) => {
    if (typeof candidate === "string") {
      return { address: candidate, kind: "lan", label: "LAN / VPN" };
    }
    return {
      address: candidate.address,
      kind: candidate.kind || "lan",
      label: candidate.label || (candidate.kind === "tailscale" ? "Tailscale" : "LAN / VPN"),
    };
  });
}

function BrandHeader({ state, mode, onModeChange, guest = false }) {
  return (
    <header className={guest ? "guest-header" : "topbar"}>
      <a className="brand" href={guest ? undefined : "/"} aria-label="Wiferry home">
        Wiferry
      </a>
      {guest ? (
        <div className="nearby-status"><span />{isTailnet(state) ? "Connected through Tailscale" : "Connected nearby"}</div>
      ) : (
        <>
          {state?.features?.receive !== false ? <nav className="mode-tabs" aria-label="Transfer mode">
            {[
              ["send", "Send"],
              ["receive", "Receive"],
            ].map(([value, label]) => (
              <button
                className={mode === value ? "active" : ""}
                key={value}
                onClick={() => onModeChange(value)}
                type="button"
              >
                {label}
              </button>
            ))}
          </nav> : <div className="send-label">Send files</div>}
          <div className="wifi-status">
            <span className={state?.active ? "status-dot" : "status-dot inactive"} />
            {state?.active ? (isTailnet(state) ? "On your tailnet" : "On this network") : "Sharing stopped"}
          </div>
        </>
      )}
    </header>
  );
}

function EmptyTransfer({ mode, onFiles, active = true }) {
  const input = useRef(null);
  const [dragging, setDragging] = useState(false);
  const receive = mode === "receive";
  return (
    <section
      className={`drop-zone${dragging ? " dragging" : ""}${receive ? " receive-zone" : ""}`}
      onDragEnter={(event) => { event.preventDefault(); setDragging(true); }}
      onDragOver={(event) => event.preventDefault()}
      onDragLeave={() => setDragging(false)}
      onDrop={(event) => {
        event.preventDefault();
        setDragging(false);
        if (!receive) onFiles([...event.dataTransfer.files]);
      }}
    >
      <div className="drop-icon"><Icon name={receive ? "arrowDown" : "folder"} size={30} /></div>
      <h1>{receive ? (active ? "Ready to receive" : "Sharing is stopped") : "Drop files here"}</h1>
      <p>
        {receive
          ? (active
            ? "Nearby devices can upload directly into your receive folder."
            : "Start sharing when you are ready to receive another file.")
          : "Local copy up to 2 GB. Use a path for larger files."}
      </p>
      {receive && active ? (
        <div className="waiting-line"><span />Waiting for a nearby device</div>
      ) : receive ? null : (
        <button className="primary-button" type="button" onClick={() => input.current?.click()}>
          <Icon name="add" /> Add files
        </button>
      )}
      <input
        ref={input}
        hidden
        multiple
        type="file"
        onChange={(event) => onFiles([...event.target.files])}
      />
    </section>
  );
}

function FileList({ files, onRemove, onClear, uploadProgress }) {
  if (!files.length && uploadProgress == null) return null;
  return (
    <section className="file-section" aria-live="polite">
      <div className="section-heading">
        <h2>Files to send ({files.length})</h2>
        {files.length > 1 ? <button onClick={onClear} type="button">Clear all</button> : null}
      </div>
      <div className="file-list">
        {files.map((file) => (
          <div className="file-row" key={file.id}>
            <div className="file-symbol"><Icon name="file" /></div>
            <div className="file-copy"><strong>{file.name}</strong><span>{file.sizeLabel}</span></div>
            <div className="ready"><Icon name="check" size={16} />Ready</div>
            <button className="icon-button" aria-label={`Remove ${file.name}`} onClick={() => onRemove(file.id)} type="button">
              <Icon name="close" />
            </button>
          </div>
        ))}
        {uploadProgress != null ? (
          <div className="file-row uploading-row">
            <div className="file-symbol"><Icon name="arrowUp" /></div>
            <div className="file-copy"><strong>Adding files…</strong><span>{uploadProgress}% copied locally</span></div>
            <div className="progress-track"><span style={{ width: `${uploadProgress}%` }} /></div>
          </div>
        ) : null}
      </div>
      {files.length ? <div className="file-total">Total: {formatTotal(files)}</div> : null}
    </section>
  );
}

function PathEntry({ onAdd }) {
  const [value, setValue] = useState("");
  const submit = (event) => {
    event.preventDefault();
    const path = value.trim();
    if (!path) return;
    onAdd(path);
    setValue("");
  };
  return (
    <form className="path-entry" onSubmit={submit}>
      <label htmlFor="local-path">Or share a file in place</label>
      <div>
        <input id="local-path" value={value} onChange={(event) => setValue(event.target.value)} placeholder="/path/to/large-file.zip" />
        <button type="submit">Share path</button>
      </div>
    </form>
  );
}

function ConnectionPanel({ state, qrUrl, onStop, onStart, onRotate, onExpiry, onHostIp }) {
  const [copied, setCopied] = useState(false);
  const candidates = hostCandidates(state);
  const copyUrl = async () => {
    await navigator.clipboard.writeText(state.shareUrl);
    setCopied(true);
    window.setTimeout(() => setCopied(false), 1600);
  };
  return (
    <aside className="connection-panel">
      <div className="ticket-top">
        <h2>Scan to connect</h2>
        <p>{isTailnet(state) ? "Open your camera on a device connected to your tailnet." : "Open your camera. No Wiferry app needed."}</p>
        <div className={`qr-wrap${state.active ? "" : " qr-disabled"}`}>
          {qrUrl ? <img src={qrUrl} alt="QR code for this Wiferry session" /> : <div className="qr-loader" />}
        </div>
      </div>
      <div className="ticket-controls">
        <div className="control-row url-control">
          <Icon name="link" />
          <span title={state.shareUrl}>{state.shareUrl}</span>
          <button onClick={copyUrl} type="button">{copied ? "Copied" : "Copy"}</button>
        </div>
        <label className="control-row">
          <Icon name="wifi" />
          <select
            aria-label="Network address used by the QR code"
            value={state.hostIp}
            onChange={(event) => onHostIp(event.target.value)}
          >
            {candidates.map((candidate) => (
              <option key={candidate.address} value={candidate.address}>{candidate.label} · {candidate.address}</option>
            ))}
          </select>
        </label>
        {isTailnet(state) ? (
          <div className="tailnet-notice" role="note">
            Both devices need Tailscale access. Traffic remains WireGuard-encrypted if Tailscale uses a DERP relay.
          </div>
        ) : null}
        <label className={`control-row${state.features?.connectedDevices === false ? " devices-control" : ""}`}>
          <Icon name="refresh" />
          <select value={state.expiryMinutes} onChange={(event) => onExpiry(Number(event.target.value))}>
            <option value={15}>Expires in 15 min</option>
            <option value={30}>Expires in 30 min</option>
            <option value={60}>Expires in 1 hour</option>
            <option value={120}>Expires in 2 hours</option>
            <option value={0}>No expiry</option>
          </select>
        </label>
        {state.features?.connectedDevices !== false ? <div className="control-row devices-control">
          <Icon name="devices" />
          <span>{state.connectedDevices} {state.connectedDevices === 1 ? "device" : "devices"} connected</span>
          <button className="rotate-button" onClick={onRotate} type="button">New code</button>
        </div> : null}
        {state.features?.connectedDevices === false ? <button className="new-code-link" onClick={onRotate} type="button">Generate a new code</button> : null}
        <button className={state.active ? "stop-button" : "start-button"} onClick={state.active ? onStop : onStart} type="button">
          <Icon name={state.active ? "stop" : "wifi"} />
          {state.active ? "Stop sharing" : "Start sharing"}
        </button>
      </div>
    </aside>
  );
}

function ActivityRail({ activities }) {
  return (
    <section className="activity-rail">
      <div className="activity-heading"><h2>Transfer activity</h2><span>Most recent</span></div>
      {activities.length ? activities.slice(0, 4).map((item) => (
        <div className="activity-row" key={item.id}>
          <div className={`activity-icon ${item.kind}`}><Icon name={item.kind === "upload" ? "arrowDown" : "arrowUp"} /></div>
          <div><strong>{item.name}</strong><span>{item.sizeLabel}</span></div>
          <span className="activity-direction">{item.kind === "upload" ? "From" : "To"} {item.device}</span>
          <span className="complete">Completed</span>
          <time>{new Date(item.timestamp * 1000).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}</time>
        </div>
      )) : <div className="empty-activity">Transfers will appear here after a nearby device connects.</div>}
    </section>
  );
}

function AdminApp() {
  const { state, setState, error, refresh } = usePolling("/api/admin/state", true);
  const [uploadProgress, setUploadProgress] = useState(null);
  const [actionError, setActionError] = useState("");
  const [qrUrl, setQrUrl] = useState("");

  useEffect(() => {
    if (!state?.shareUrl) return undefined;
    const controller = new AbortController();
    let cancelled = false;
    let objectUrl = "";
    fetchAdminQr(controller.signal).then((blob) => {
      objectUrl = URL.createObjectURL(blob);
      if (cancelled) URL.revokeObjectURL(objectUrl);
      else setQrUrl(objectUrl);
    }).catch((reason) => {
      if (reason.name !== "AbortError") setActionError(reason.message);
    });
    return () => {
      cancelled = true;
      controller.abort();
      if (objectUrl) URL.revokeObjectURL(objectUrl);
    };
  }, [state?.shareUrl]);

  const action = async (path, options = {}) => {
    try {
      const next = await requestJson(path, options, true);
      if (next?.files || next?.mode) setState(next);
      else await refresh();
      setActionError("");
    } catch (reason) {
      setActionError(reason.message);
    }
  };

  const addFiles = async (files) => {
    if (!files.length) return;
    setUploadProgress(0);
    try {
      await uploadWithProgress("/api/admin/files", files, setUploadProgress, true);
      await refresh();
      setActionError("");
    } catch (reason) {
      setActionError(reason.message);
    } finally {
      setUploadProgress(null);
    }
  };

  const addPath = async (path) => {
    try {
      await requestJson("/api/admin/paths", { method: "POST", body: JSON.stringify({ paths: [path] }) }, true);
      await refresh();
      setActionError("");
    } catch (reason) {
      setActionError(reason.message);
    }
  };

  if (!state) return <LoadingScreen error={error} />;
  return (
    <div className="app-shell">
      <BrandHeader
        state={state}
        mode={state.mode}
        onModeChange={(mode) => action("/api/admin/mode", { method: "POST", body: JSON.stringify({ mode }) })}
      />
      {(error || actionError) ? <div className="error-banner" role="alert">{actionError || error}</div> : null}
      <main className="admin-main">
        <div className="workspace-left">
          <EmptyTransfer mode={state.mode} onFiles={addFiles} active={state.active} />
          {state.features?.pathEntry !== false && state.mode === "send" ? <PathEntry onAdd={addPath} /> : null}
          {state.mode === "send" ? (
            <FileList
              files={state.files}
              uploadProgress={uploadProgress}
              onRemove={(id) => action(`/api/admin/files/${id}`, { method: "DELETE" })}
              onClear={() => action("/api/admin/files", { method: "DELETE" })}
            />
          ) : (
            <div className="receive-path"><span>Saving received files to</span><strong>{state.receiveDir}</strong></div>
          )}
        </div>
        <ConnectionPanel
          state={state}
          qrUrl={qrUrl}
          onStop={() => action("/api/admin/stop", { method: "POST" })}
          onStart={() => action("/api/admin/start", { method: "POST" })}
          onRotate={() => action("/api/admin/rotate", { method: "POST" })}
          onExpiry={(minutes) => action("/api/admin/expiry", { method: "POST", body: JSON.stringify({ minutes }) })}
          onHostIp={(address) => action("/api/admin/host-ip", { method: "POST", body: JSON.stringify({ address }) })}
        />
      </main>
      {state.features?.activity !== false ? <ActivityRail activities={state.activities} /> : null}
      <footer className="privacy-footer"><Icon name="shield" />{isTailnet(state) ? "No Wiferry cloud storage or file relay. Tailscale carries the traffic." : "Files stay on your local network."}</footer>
    </div>
  );
}

function GuestFileRow({ file, token }) {
  return (
    <div className="guest-file-row">
      <div className="audio-symbol"><Icon name="file" size={26} /></div>
      <div className="guest-file-copy"><strong>{file.name}</strong><span>{file.sizeLabel}</span></div>
      <a className="download-action" href={`/api/session/${token}/files/${file.id}`} download={file.name}>Download</a>
    </div>
  );
}

function GuestApp() {
  const path = `/api/session/${guestToken}`;
  const { state, error, refresh } = usePolling(path, false);
  const [progress, setProgress] = useState(null);
  const [uploadMessage, setUploadMessage] = useState("");
  const input = useRef(null);

  const upload = async (files) => {
    if (!files.length) return;
    setProgress(0);
    setUploadMessage("");
    try {
      const result = await uploadWithProgress(`${path}/upload`, files, setProgress);
      setUploadMessage(`${result.files.length} ${result.files.length === 1 ? "file" : "files"} sent`);
      await refresh();
    } catch (reason) {
      setUploadMessage(reason.message);
    } finally {
      setProgress(null);
    }
  };

  if (error && /ended|not found/i.test(error)) return <EndedScreen />;
  if (!state) return <LoadingScreen error={error} />;
  return (
    <div className="guest-shell">
      <BrandHeader state={state} guest />
      <main className="guest-main">
        <section className="guest-intro">
          <h1>{state.canDownload ? `Files from ${state.deviceName}` : `Send to ${state.deviceName}`}</h1>
          <p>{isTailnet(state) ? "Through your encrypted Tailscale network" : "Direct over your local network"}</p>
        </section>
        {state.canDownload ? (
          <>
            <section className="guest-file-list">
              {state.files.length ? state.files.map((file) => (
                <GuestFileRow file={file} token={guestToken} key={file.id} />
              )) : <div className="guest-empty">The sender has not added any files yet.</div>}
            </section>
            {state.files.length > 1 && state.features?.downloadAll !== false ? (
              <a className="download-all" href={`${path}/download-all`} download="Wiferry files.zip">
                <Icon name="arrowDown" />Download all
              </a>
            ) : null}
          </>
        ) : null}
        {state.canUpload ? (
          <section className="send-back">
            <h2>Send something back</h2>
            <p>Share files from your device to {state.deviceName}.</p>
            <button onClick={() => input.current?.click()} type="button">
              <Icon name="arrowUp" />{progress == null ? "Choose files" : `Sending… ${progress}%`}
            </button>
            <input ref={input} hidden multiple type="file" onChange={(event) => upload([...event.target.files])} />
            {progress != null ? <div className="guest-progress"><span style={{ width: `${progress}%` }} /></div> : null}
            {uploadMessage ? <div className="upload-message" role="status">{uploadMessage}</div> : null}
          </section>
        ) : null}
        {error ? <div className="error-banner" role="alert">{error}</div> : null}
      </main>
      <footer className="guest-footer">No Wiferry cloud storage. {formatCountdown(state.secondsRemaining)}.</footer>
    </div>
  );
}

function EndedScreen() {
  return (
    <div className="guest-shell">
      <header className="guest-header"><div className="brand">Wiferry</div></header>
      <main className="ended-screen">
        <div className="ended-icon"><Icon name="stop" size={28} /></div>
        <h1>This share has ended</h1>
        <p>Ask the sender to start a new session and scan the new QR code.</p>
      </main>
      <footer className="guest-footer">No Wiferry cloud storage. No files remain available from this link.</footer>
    </div>
  );
}

function LoadingScreen({ error }) {
  return (
    <main className="loading-screen">
      <div className="brand">Wiferry</div>
      <div className="loading-line" />
      <p>{error || "Connecting to Wiferry…"}</p>
    </main>
  );
}

export default function App() {
  return isGuest ? <GuestApp /> : <AdminApp />;
}
