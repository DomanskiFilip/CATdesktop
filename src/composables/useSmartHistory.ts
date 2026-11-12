import { ref, computed } from "vue";

// History of smart feature runs (email generations / route plans).
// Shared between the calendar's SmartFeatures popup and the Smart Features tab.
// Persisted in localStorage, capped at MAX_HISTORY (oldest dropped first).

export const MAX_HISTORY = 10;
const STORAGE_KEY = "smartFeaturesHistory";

export type TravelMode = "car" | "transit";

interface SmartHistoryBase {
  id: string;
  createdAt: string; // ISO timestamp of when it was generated/last updated
  title: string;
  eventTime: string | null; // "YYYY-MM-DDTHH:mm" (local) used when adding as event
  addedEventId: string | null; // set once the entry was turned into a calendar event
  sourceEventId?: string | null; // calendar event it was generated from, if any
}

export interface EmailHistoryEntry extends SmartHistoryBase {
  type: "email";
  email: {
    topic: string;
    subject: string;
    body: string;
    participants: string[];
  };
}

export interface RouteHistoryEntry extends SmartHistoryBase {
  type: "route";
  route: {
    origin: string; // address or "lat,lng"
    originLabel: string; // what to show the user
    destination: string;
    mode: TravelMode;
    durationSeconds: number | null;
    durationText: string | null;
    distanceText: string | null;
  };
}

export type SmartHistoryEntry = EmailHistoryEntry | RouteHistoryEntry;

function load(): SmartHistoryEntry[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    return Array.isArray(parsed) ? parsed.slice(0, MAX_HISTORY) : [];
  } catch {
    return [];
  }
}

const history = ref<SmartHistoryEntry[]>(load());

function persist() {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(history.value));
  } catch (e) {
    console.warn("Failed to persist smart features history:", e);
  }
}

// Insert or update an entry. New entries (and updates with moveToFront) go to
// the top; anything beyond MAX_HISTORY is dropped from the bottom (oldest).
function upsert(entry: SmartHistoryEntry, moveToFront = true) {
  const list = history.value.slice();
  const idx = list.findIndex((e) => e.id === entry.id);
  const copy = JSON.parse(JSON.stringify(entry)) as SmartHistoryEntry;
  if (idx === -1) {
    list.unshift(copy);
  } else if (moveToFront) {
    list.splice(idx, 1);
    list.unshift(copy);
  } else {
    list[idx] = copy;
  }
  history.value = list.slice(0, MAX_HISTORY);
  persist();
}

function remove(id: string) {
  history.value = history.value.filter((e) => e.id !== id);
  persist();
}

function clear() {
  history.value = [];
  persist();
}

function get(id: string): SmartHistoryEntry | undefined {
  return history.value.find((e) => e.id === id);
}

// Keep tabs/windows in sync
window.addEventListener("storage", (e) => {
  if (e.key === STORAGE_KEY) history.value = load();
});

export function useSmartHistory() {
  return {
    history: computed(() => history.value),
    upsert,
    remove,
    clear,
    get,
  };
}
