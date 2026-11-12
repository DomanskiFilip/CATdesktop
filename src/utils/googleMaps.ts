// Shared Google Maps JS API loader (used by SmartFeatures and SmartFeaturesHub)
const apiKey = (import.meta.env.VITE_GOOGLE_MAPS_API_KEY as string) || "";

export const MAPS_KEY_MISSING_MSG =
  "Google Maps API key is missing. Add VITE_GOOGLE_MAPS_API_KEY to a .env file in the project root and restart the app.";
export const MAPS_AUTH_FAILED_MSG =
  "Google Maps rejected the API key. Check that the Maps JavaScript API and Directions API are enabled, billing is active, and the key's allowed referrers include this app (http://localhost:5173 in dev, http://tauri.localhost and tauri://localhost in builds).";

if (!apiKey && import.meta.env.DEV) {
  console.warn(MAPS_KEY_MISSING_MSG);
}

// Google calls window.gm_authFailure when the key is invalid/restricted.
let authFailed = false;
const authFailureListeners = new Set<() => void>();
(window as any).gm_authFailure = () => {
  authFailed = true;
  console.error(MAPS_AUTH_FAILED_MSG);
  authFailureListeners.forEach((cb) => cb());
};

// Register a callback for key/auth failures. Returns an unsubscribe function.
export function onMapsAuthFailure(cb: () => void): () => void {
  if (authFailed) cb();
  authFailureListeners.add(cb);
  return () => authFailureListeners.delete(cb);
}

let loadingPromise: Promise<void> | null = null;

export function loadGoogleMapsApi(): Promise<void> {
  if (!apiKey) return Promise.reject(new Error(MAPS_KEY_MISSING_MSG));
  if (authFailed) return Promise.reject(new Error(MAPS_AUTH_FAILED_MSG));
  if (window.google && window.google.maps) return Promise.resolve();
  if (loadingPromise) return loadingPromise;

  loadingPromise = new Promise<void>((resolve, reject) => {
    const existingScript = document.getElementById("google-maps-script");
    if (existingScript) {
      existingScript.addEventListener("load", () => resolve());
      existingScript.addEventListener("error", (e) => reject(e));
      return;
    }
    const script = document.createElement("script");
    script.id = "google-maps-script";
    script.src = `https://maps.googleapis.com/maps/api/js?key=${apiKey}&libraries=places`;
    script.async = true;
    script.defer = true;
    script.onload = () => resolve();
    script.onerror = () => {
      loadingPromise = null; // allow retry
      script.remove();
      reject(
        new Error("Could not load Google Maps (check your internet connection)."),
      );
    };
    document.head.appendChild(script);
  });
  return loadingPromise;
}
