/* Gọi các endpoint HTTP của backend. */

async function request(url, options = {}) {
  const response = await fetch(url, {
    headers: { "Content-Type": "application/json" },
    ...options,
  });

  let payload = null;
  try {
    payload = await response.json();
  } catch (_) {
    payload = null;
  }

  if (!response.ok) {
    throw new Error(payload?.error || `HTTP ${response.status}`);
  }
  return payload;
}

export const api = {
  listProfiles: () => request("/api/profiles"),

  saveProfile: (profile) => request("/api/profiles", {
    method: "POST",
    body: JSON.stringify(profile),
  }),

  deleteProfile: (id) => request(`/api/profiles/${encodeURIComponent(id)}`, {
    method: "DELETE",
  }),

  markUsed: (id) => request(`/api/profiles/${encodeURIComponent(id)}/used`, {
    method: "POST",
    body: "{}",
  }),

  cancelRun: (jobId) => request("/api/test/cancel", {
    method: "POST",
    body: JSON.stringify({ job_id: jobId }),
  }),
};
