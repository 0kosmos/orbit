// "Check for updates": asks GitHub for Orbit's newest release and compares it with
// the running version. Only runs when the user clicks (no background checks).
import { getVersion } from "@tauri-apps/api/app";
import { invoke } from "@tauri-apps/api/core";

export const REPO_URL = "https://github.com/0kosmos/orbit";
const API_URL = "https://api.github.com/repos/0kosmos/orbit/releases/latest";

export const update = $state({
  status: "idle" as "idle" | "checking" | "latest" | "available" | "error",
  current: "",
  latest: "",
  url: "",
  message: "",
});

/** "1.2.10" > "1.2.9" */
function isNewer(a: string, b: string) {
  const pa = a.split(".").map((n) => parseInt(n) || 0);
  const pb = b.split(".").map((n) => parseInt(n) || 0);
  for (let i = 0; i < Math.max(pa.length, pb.length); i++) {
    if ((pa[i] ?? 0) !== (pb[i] ?? 0)) return (pa[i] ?? 0) > (pb[i] ?? 0);
  }
  return false;
}

export async function checkForUpdates() {
  if (update.status === "checking") return;
  update.status = "checking";
  update.message = "";
  try {
    update.current = await getVersion();
    const res = await fetch(API_URL, { headers: { Accept: "application/vnd.github+json" } });
    if (res.status === 404) {
      // No release yet (or the repository is private).
      update.status = "error";
      update.message = "No releases published yet.";
      return;
    }
    if (!res.ok) throw new Error(`GitHub answered ${res.status}`);
    const release = await res.json();
    update.latest = String(release.tag_name ?? "").replace(/^v/i, "");
    update.url = String(release.html_url ?? `${REPO_URL}/releases`);
    update.status = isNewer(update.latest, update.current) ? "available" : "latest";
  } catch {
    update.status = "error";
    update.message = "Couldn't reach GitHub. Check your internet connection.";
  }
}

/** Open the release page (or any page of Orbit's repository). */
export function openGithub(url = `${REPO_URL}/releases`) {
  return invoke("open_github", { url });
}
