const metaToken = document
  .querySelector('meta[name="wiferry-admin-token"]')
  ?.getAttribute("content") || "";

let fragmentToken = "";
let storedToken;
if (!window.location.pathname.startsWith("/s/") && window.location.hash.length > 1) {
  try {
    fragmentToken = decodeURIComponent(window.location.hash.slice(1));
  } catch {
    fragmentToken = "";
  }
  if (fragmentToken) {
    try {
      window.sessionStorage.setItem("wiferryAdminToken", fragmentToken);
    } catch {
      // The fragment still keeps this tab usable when session storage is disabled.
    }
    window.history.replaceState(null, "", `${window.location.pathname}${window.location.search}`);
  }
}
try {
  storedToken = window.sessionStorage.getItem("wiferryAdminToken") || "";
} catch {
  storedToken = "";
}

const adminToken = fragmentToken || metaToken || storedToken;

export const isGuest = window.location.pathname.startsWith("/s/");
export const guestToken = isGuest ? window.location.pathname.split("/")[2] : "";

export async function requestJson(path, options = {}, admin = false) {
  const headers = new Headers(options.headers || {});
  if (admin && adminToken) headers.set("X-Wiferry-Admin", adminToken);
  if (options.body && !(options.body instanceof FormData)) {
    headers.set("Content-Type", "application/json");
  }
  const response = await fetch(path, { ...options, headers });
  if (!response.ok) {
    let detail = `Request failed (${response.status})`;
    try {
      const payload = await response.json();
      detail = payload.detail || detail;
    } catch {
      // Keep the status-based fallback.
    }
    throw new Error(detail);
  }
  return response.json();
}

export function uploadWithProgress(path, files, onProgress, admin = false) {
  return new Promise((resolve, reject) => {
    const request = new XMLHttpRequest();
    request.open("POST", path);
    if (admin && adminToken) request.setRequestHeader("X-Wiferry-Admin", adminToken);
    request.responseType = "json";
    request.upload.addEventListener("progress", (event) => {
      if (event.lengthComputable) onProgress(Math.round((event.loaded / event.total) * 100));
    });
    request.addEventListener("load", () => {
      if (request.status >= 200 && request.status < 300) resolve(request.response);
      else reject(new Error(request.response?.detail || `Upload failed (${request.status})`));
    });
    request.addEventListener("error", () => reject(new Error("The upload connection was interrupted")));
    const body = new FormData();
    for (const file of files) body.append("files", file, file.name);
    request.send(body);
  });
}

export async function fetchAdminQr(signal) {
  const response = await fetch("/api/admin/qr", {
    headers: { "X-Wiferry-Admin": adminToken || "" },
    signal,
  });
  if (!response.ok) throw new Error("Could not generate QR code");
  return response.blob();
}
