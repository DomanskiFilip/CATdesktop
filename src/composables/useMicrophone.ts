import { ref, computed } from "vue";
import { invoke } from "@tauri-apps/api/core";

// Shared microphone state: access permission + chosen input device.
// Used by the mic button in the AI assistant and by the Settings tab.

export type MicPermission = "unknown" | "granted" | "denied" | "unsupported";

const ALLOWED_KEY = "microphoneAccessAllowed";
const DEVICE_KEY = "selectedMicrophoneId";

const isLinux = /Linux/i.test(navigator.userAgent) && !/Android/i.test(navigator.userAgent);

const devices = ref<MediaDeviceInfo[]>([]);
// "" = system default
const selectedDeviceId = ref<string>(localStorage.getItem(DEVICE_KEY) || "");
const permission = ref<MicPermission>("unknown");
const error = ref("");
const busy = ref(false);
let initPromise: Promise<void> | null = null;

const isSupported = () => !!navigator.mediaDevices?.getUserMedia;

const UNSUPPORTED_MSG = isLinux
  ? "Microphone capture isn't available in this window. Restart the app; if it still fails, install the GStreamer PipeWire/PulseAudio plugins (gst-plugins-good, gst-plugin-pipewire)."
  : "Microphone capture isn't supported on this device.";

function describeError(err: unknown): string {
  const name = (err as { name?: string })?.name;
  switch (name) {
    case "NotAllowedError":
    case "SecurityError":
      return isLinux
        ? "Microphone access was denied. Click “Allow microphone access” and try again."
        : "Microphone access was denied. Allow it in the prompt or your system privacy settings.";
    case "NotFoundError":
      return "No microphone found. Plug one in and press refresh.";
    case "NotReadableError":
      return "The microphone is busy or unavailable (another app may be using it).";
    case "OverconstrainedError":
      return "The selected microphone is no longer available.";
    default:
      return `Microphone error: ${name || String(err)}`;
  }
}

async function syncBackend(allowed: boolean) {
  try {
    await invoke("set_microphone_permission", { allowed });
  } catch (e) {
    console.warn("Failed to sync microphone permission:", e);
  }
}

async function refreshDevices() {
  if (!isSupported()) return;
  try {
    const all = await navigator.mediaDevices.enumerateDevices();
    devices.value = all.filter((d) => d.kind === "audioinput" && d.deviceId);
    if (
      selectedDeviceId.value &&
      devices.value.length &&
      !devices.value.some((d) => d.deviceId === selectedDeviceId.value)
    ) {
      // Saved device was unplugged -> fall back to system default
      selectDevice("");
    }
  } catch (e) {
    console.warn("Failed to enumerate microphones:", e);
  }
}

function audioConstraint(deviceId: string): MediaTrackConstraints | boolean {
  return deviceId ? { deviceId: { exact: deviceId } } : true;
}

// Ask for microphone access (shows the system prompt where one exists) and
// load the device list. Returns true when access is granted.
async function requestAccess(): Promise<boolean> {
  error.value = "";
  if (!isSupported()) {
    permission.value = "unsupported";
    error.value = UNSUPPORTED_MSG;
    return false;
  }
  busy.value = true;
  try {
    await syncBackend(true);
    let stream: MediaStream;
    try {
      stream = await navigator.mediaDevices.getUserMedia({
        audio: audioConstraint(selectedDeviceId.value),
      });
    } catch (e) {
      const name = (e as { name?: string })?.name;
      if (
        selectedDeviceId.value &&
        (name === "OverconstrainedError" || name === "NotFoundError")
      ) {
        selectDevice("");
        stream = await navigator.mediaDevices.getUserMedia({ audio: true });
      } else {
        throw e;
      }
    }
    stream.getTracks().forEach((t) => t.stop());
    permission.value = "granted";
    localStorage.setItem(ALLOWED_KEY, "true");
    await refreshDevices();
    return true;
  } catch (e) {
    console.error("Microphone access failed:", e);
    const name = (e as { name?: string })?.name;
    if (name === "NotAllowedError" || name === "SecurityError") {
      permission.value = "denied";
      localStorage.removeItem(ALLOWED_KEY);
      await syncBackend(false);
    }
    error.value = describeError(e);
    return false;
  } finally {
    busy.value = false;
  }
}

function selectDevice(deviceId: string) {
  selectedDeviceId.value = deviceId;
  if (deviceId) localStorage.setItem(DEVICE_KEY, deviceId);
  else localStorage.removeItem(DEVICE_KEY);
}

// Open a stream on the selected device, falling back to the default one if it vanished.
async function openStream(
  extra: MediaTrackConstraints = {},
): Promise<MediaStream> {
  const build = (id: string): MediaStreamConstraints => ({
    audio: { ...(id ? { deviceId: { exact: id } } : {}), ...extra },
  });
  try {
    return await navigator.mediaDevices.getUserMedia(build(selectedDeviceId.value));
  } catch (e) {
    const name = (e as { name?: string })?.name;
    if (
      selectedDeviceId.value &&
      (name === "OverconstrainedError" || name === "NotFoundError")
    ) {
      selectDevice("");
      return navigator.mediaDevices.getUserMedia(build(""));
    }
    throw e;
  }
}

function init(): Promise<void> {
  if (initPromise) return initPromise;
  initPromise = (async () => {
    if (!isSupported()) {
      permission.value = "unsupported";
      error.value = UNSUPPORTED_MSG;
      return;
    }
    navigator.mediaDevices.addEventListener?.("devicechange", refreshDevices);
    if (localStorage.getItem(ALLOWED_KEY) === "true") {
      // Previously allowed: re-arm the backend and reload labelled devices
      await requestAccess();
    } else {
      await refreshDevices();
    }
  })();
  return initPromise;
}

const selectedLabel = computed(() => {
  if (!selectedDeviceId.value) return "System default";
  const d = devices.value.find((x) => x.deviceId === selectedDeviceId.value);
  return d?.label || "Selected microphone";
});

export function micLabel(d: MediaDeviceInfo, index: number) {
  return d.label || `Microphone ${index + 1}`;
}

export function useMicrophone() {
  return {
    devices,
    selectedDeviceId,
    selectedLabel,
    permission,
    error,
    busy,
    isLinux,
    init,
    requestAccess,
    refreshDevices,
    selectDevice,
    openStream,
    isSupported,
  };
}
