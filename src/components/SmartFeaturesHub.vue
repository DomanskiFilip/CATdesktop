<template>
  <section id="smart-hub" :class="{ 'is-mobile': isMobile }">
    <!-- ===== History column ===== -->
    <aside class="hub-history">
      <header class="hub-history-header">
        <h2>Smart Features</h2>
        <button class="primary-btn" @click="startNew()" title="Start new">
          + New
        </button>
      </header>
      <div class="hub-history-sub">
        <span>Recent ({{ history.length }}/{{ MAX_HISTORY }})</span>
        <button
          v-if="history.length"
          class="link-btn"
          @click="clearHistory"
          title="Clear history"
        >
          {{ confirmClear ? "Click again to clear" : "Clear" }}
        </button>
      </div>
      <p v-if="!history.length" class="empty-hint">
        No emails or routes yet. Generated emails and planned routes will show
        up here (the last {{ MAX_HISTORY }} are kept).
      </p>
      <ul class="history-list">
        <li
          v-for="entry in history"
          :key="entry.id"
          :class="{ active: current?.id === entry.id }"
          @click="openEntry(entry)"
        >
          <span class="history-icon" :title="entry.type">
            <svg
              v-if="entry.type === 'email'"
              xmlns="http://www.w3.org/2000/svg"
              height="20px"
              viewBox="0 -960 960 960"
              width="20px"
              fill="var(--color-theme)"
            >
              <path
                d="M160-160q-33 0-56.5-23.5T80-240v-480q0-33 23.5-56.5T160-800h640q33 0 56.5 23.5T880-720v480q0 33-23.5 56.5T800-160H160Zm320-280L160-640v400h640v-400L480-440Zm0-80 320-200H160l320 200ZM160-640v-80 480-400Z"
              />
            </svg>
            <svg
              v-else
              xmlns="http://www.w3.org/2000/svg"
              height="20px"
              viewBox="0 -960 960 960"
              width="20px"
              fill="var(--color-theme)"
            >
              <path
                d="M360-120q-66 0-113-47t-47-113v-327q-35-13-57.5-43.5T120-720q0-50 35-85t85-35q50 0 85 35t35 85q0 39-22.5 69.5T280-607v327q0 33 23.5 56.5T360-200q33 0 56.5-23.5T440-280v-400q0-66 47-113t113-47q66 0 113 47t47 113v327q35 13 57.5 43.5T840-240q0 50-35 85t-85 35q-50 0-85-35t-35-85q0-39 22.5-70t57.5-43v-327q0-33-23.5-56.5T600-800q-33 0-56.5 23.5T520-720v400q0 66-47 113t-113 47Z"
              />
            </svg>
          </span>
          <span class="history-text">
            <span class="history-title">{{ entry.title }}</span>
            <span class="history-meta">{{ entryMeta(entry) }}</span>
            <span class="history-meta">
              {{ timeAgo(entry.createdAt) }}
              <span v-if="entry.addedEventId" class="in-calendar"
                >· in calendar</span
              >
            </span>
          </span>
          <button
            class="icon-btn history-delete"
            title="Remove from history"
            @click.stop="removeEntry(entry.id)"
          >
            <svg
              xmlns="http://www.w3.org/2000/svg"
              height="18px"
              viewBox="0 -960 960 960"
              width="18px"
              fill="var(--color-text)"
            >
              <path
                d="m256-200-56-56 224-224-224-224 56-56 224 224 224-224 56 56-224 224 224 224-56 56-224-224-224 224Z"
              />
            </svg>
          </button>
        </li>
      </ul>
    </aside>

    <!-- ===== Workspace ===== -->
    <section class="hub-workspace">
      <!-- New: pick type + form -->
      <template v-if="!current">
        <div class="tabs">
          <button
            :class="{ active: newType === 'email' }"
            @click="newType = 'email'"
          >
            Email
          </button>
          <button
            :class="{ active: newType === 'route' }"
            @click="newType = 'route'"
          >
            Route
          </button>
        </div>

        <!-- New email -->
        <form
          v-if="newType === 'email'"
          class="hub-form"
          @submit.prevent="generateEmail"
        >
          <h3>New email</h3>
          <label>Recipients</label>
          <div class="chips" v-if="emailParticipants.length">
            <span v-for="p in emailParticipants" :key="p" class="chip">
              {{ p }}
              <button type="button" @click="removeParticipant(p)">×</button>
            </span>
          </div>
          <div class="row">
            <input
              v-model="participantInput"
              placeholder="name@example.com (Enter to add)"
              @keydown.enter.prevent="addParticipant"
              @blur="participantInput.trim() && addParticipant()"
            />
            <button type="button" class="secondary-btn" @click="addParticipant">
              Add
            </button>
          </div>
          <label>What is the email about?</label>
          <textarea
            v-model="emailTopic"
            rows="3"
            placeholder="e.g. Invite the team to a project kickoff on Friday, ask them to bring their Q4 ideas"
          ></textarea>
          <label>Related date &amp; time</label>
          <input v-model="emailTime" type="datetime-local" />
          <span v-if="emailFormError" class="error">{{ emailFormError }}</span>
          <button
            type="submit"
            class="primary-btn"
            :disabled="emailLoading"
          >
            {{ emailLoading ? "Generating..." : "Generate email" }}
          </button>
          <div v-if="emailLoading" id="loader"></div>
        </form>

        <!-- New route -->
        <form v-else class="hub-form" @submit.prevent="planRoute">
          <h3>New route</h3>
          <label>From</label>
          <input
            v-model="routeOrigin"
            :placeholder="
              coordinates
                ? 'Current location (leave empty) or enter an address'
                : 'Enter start location'
            "
          />
          <label>To</label>
          <input
            v-model="routeDestination"
            placeholder="e.g. London Eye, London"
          />
          <label>Travel by</label>
          <div class="row mode-row">
            <button
              type="button"
              :class="{ active: routeMode === 'car' }"
              @click="routeMode = 'car'"
            >
              Car
            </button>
            <button
              type="button"
              :class="{ active: routeMode === 'transit' }"
              @click="routeMode = 'transit'"
            >
              Public transport
            </button>
          </div>
          <label>Arrive by</label>
          <input v-model="routeTime" type="datetime-local" />
          <span v-if="routeFormError" class="error">{{ routeFormError }}</span>
          <button
            type="submit"
            class="primary-btn"
            :disabled="routeLoading"
          >
            {{ routeLoading ? "Planning..." : "Plan route" }}
          </button>
          <div v-if="routeLoading" id="loader"></div>
        </form>
      </template>

      <!-- Opened email -->
      <section v-else-if="currentEmail" class="hub-detail">
        <header class="detail-header">
          <button class="link-btn" @click="startNew('email')">
            ← New email
          </button>
        </header>
        <div class="detail-field">
          <label>To</label>
          <div class="chips">
            <span
              v-for="p in currentEmail.email.participants"
              :key="p"
              class="chip"
              >{{ p }}</span
            >
          </div>
        </div>
        <div class="detail-field">
          <label>Subject</label>
          <input
            v-model="currentEmail.email.subject"
            @change="saveCurrent"
          />
        </div>
        <div class="detail-field body-field">
          <label>Body</label>
          <button
            class="icon-btn copy-btn"
            title="Copy email"
            @click="copyEmail"
          >
            <svg
              xmlns="http://www.w3.org/2000/svg"
              height="20px"
              viewBox="0 -960 960 960"
              width="20px"
              fill="var(--color-text)"
            >
              <path
                d="M360-240q-29.7 0-50.85-21.15Q288-282.3 288-312v-480q0-29.7 21.15-50.85Q330.3-864 360-864h384q29.7 0 50.85 21.15Q816-821.7 816-792v480q0 29.7-21.15 50.85Q773.7-240 744-240H360Zm0-72h384v-480H360v480ZM216-96q-29.7 0-50.85-21.15Q144-138.3 144-168v-552h72v552h456v72H216Zm144-216v-480 480Z"
              />
            </svg>
          </button>
          <textarea
            v-model="currentEmail.email.body"
            rows="10"
            @change="saveCurrent"
          ></textarea>
          <span v-if="copied" class="success">Copied!</span>
        </div>
        <div class="row">
          <button class="secondary-btn" @click="openMailClient">
            Open in email app
          </button>
          <span v-if="mailInfo" class="success">{{ mailInfo }}</span>
        </div>
      </section>

      <!-- Opened route -->
      <section v-else-if="currentRoute" class="hub-detail">
        <header class="detail-header">
          <button class="link-btn" @click="startNew('route')">
            ← New route
          </button>
        </header>
        <p class="route-summary">
          <strong>{{ currentRoute.route.originLabel }}</strong>
          →
          <strong>{{ currentRoute.route.destination }}</strong>
          <template v-if="currentRoute.route.durationText">
            <br />
            {{ currentRoute.route.durationText }}
            <span v-if="currentRoute.route.distanceText"
              >· {{ currentRoute.route.distanceText }}</span
            >
          </template>
        </p>
        <div class="row mode-row">
          <button
            :class="{ active: currentRoute.route.mode === 'car' }"
            @click="changeRouteMode('car')"
          >
            Car
          </button>
          <button
            :class="{ active: currentRoute.route.mode === 'transit' }"
            @click="changeRouteMode('transit')"
          >
            Public transport
          </button>
        </div>
        <span v-if="routeError" class="error">{{ routeError }}</span>
        <div v-show="!routeError" ref="mapEl" class="hub-map"></div>
        <div v-if="transitSteps.length" class="transit-steps">
          <h4>Transit steps</h4>
          <ol>
            <li v-for="(step, idx) in transitSteps" :key="idx">
              <template v-if="step.travel_mode === 'TRANSIT' && step.transit">
                <span class="steps-color">Take</span>
                {{
                  step.transit.line?.short_name ||
                  step.transit.line?.name ||
                  "transit"
                }}
                <span v-if="step.transit.headsign"
                  >towards {{ step.transit.headsign }}</span
                >
                <span class="steps-color">from</span>
                {{ step.transit.departure_stop?.name }}
                <span class="steps-color">to</span>
                {{ step.transit.arrival_stop?.name }}
                <span v-if="step.transit.departure_time">
                  ({{ step.transit.departure_time.text }} –
                  {{ step.transit.arrival_time?.text }})</span
                >
              </template>
              <template v-else>
                <span class="steps-color">Walk:</span>
                {{ stripHtml(step.instructions) }}
                ({{ step.duration?.text }})
              </template>
            </li>
          </ol>
        </div>
      </section>

      <!-- Add as event (shared by email & route) -->
      <section v-if="current" class="add-event">
        <h4>Add as calendar event</h4>
        <template v-if="current.addedEventId">
          <span class="success">✓ Added to your calendar</span>
        </template>
        <template v-else>
          <div class="row">
            <input
              v-model="eventTitle"
              class="grow"
              placeholder="Event title"
            />
            <input
              v-model="current.eventTime"
              type="datetime-local"
              @change="saveCurrent"
            />
          </div>
          <label v-if="currentRoute" class="checkbox-label">
            <input
              v-model="departureReminder"
              type="checkbox"
              :disabled="!currentRoute.route.durationSeconds"
            />
            Smart departure reminder (notify me when it's time to leave)
          </label>
          <span v-if="conflictWarning" class="error">{{
            conflictWarning
          }}</span>
          <span v-if="addEventError" class="error">{{ addEventError }}</span>
          <button
            class="primary-btn"
            :disabled="addingEvent"
            @click="addAsEvent"
          >
            {{
              addingEvent
                ? "Adding..."
                : conflictWarning
                  ? "Add anyway"
                  : "Add to calendar"
            }}
          </button>
        </template>
        <span v-if="addEventInfo" class="success">{{ addEventInfo }}</span>
      </section>
    </section>
  </section>
</template>

<reference types="google.maps" />
<script setup lang="ts">
import { ref, computed, nextTick, watch, onBeforeUnmount } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { emit as tauriEmit } from "@tauri-apps/api/event";
import { platform } from "@tauri-apps/plugin-os";
import { formatDistanceToNow } from "date-fns";
import {
  loadGoogleMapsApi,
  onMapsAuthFailure,
  MAPS_AUTH_FAILED_MSG,
} from "../utils/googleMaps";
import {
  useSmartHistory,
  MAX_HISTORY,
  type SmartHistoryEntry,
  type EmailHistoryEntry,
  type RouteHistoryEntry,
  type TravelMode,
} from "../composables/useSmartHistory";

const props = defineProps<{
  coordinates: { lat: number; lng: number } | null;
}>();

const isMobile =
  /Android|webOS|iPhone|iPad|iPod|BlackBerry|IEMobile|Opera Mini/i.test(
    navigator.userAgent,
  );

const { history, upsert, remove, clear } = useSmartHistory();

// == helpers == //
const pad = (n: number) => String(n).padStart(2, "0");
function toLocalInput(d: Date): string {
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}T${pad(d.getHours())}:${pad(d.getMinutes())}`;
}
function nextHourLocal(): string {
  const d = new Date();
  d.setMinutes(0, 0, 0);
  d.setHours(d.getHours() + 1);
  return toLocalInput(d);
}
function timeAgo(iso: string) {
  try {
    return formatDistanceToNow(new Date(iso), { addSuffix: true });
  } catch {
    return "";
  }
}
function stripHtml(s?: string) {
  return s ? s.replace(/<[^>]+>/g, "") : "";
}
function errMessage(err: unknown, fallback: string) {
  if (err instanceof Error) return err.message;
  if (typeof err === "string") return err;
  return fallback;
}
function entryMeta(entry: SmartHistoryEntry) {
  if (entry.type === "email") {
    return `to ${entry.email.participants.join(", ")}`;
  }
  const mode = entry.route.mode === "car" ? "Car" : "Transit";
  return entry.route.durationText
    ? `${mode} · ${entry.route.durationText}`
    : `${mode} · to ${entry.route.destination}`;
}
function buildCalendarEvent(
  description: string,
  localTime: string,
  participants: string[] = [],
) {
  const date = new Date(localTime);
  return {
    id: crypto.randomUUID(),
    user_id: "",
    description,
    time: date.toISOString(),
    alarm: false,
    synced: false,
    synced_google: false,
    synced_outlook: false,
    deleted: false,
    recurrence: null,
    participants,
  };
}
// On mobile the backend needs the user id before saving events
async function provideMobileUserId() {
  try {
    const os = await platform();
    if (os === "android" || os === "ios") {
      const userId = localStorage.getItem("cachedUserId");
      if (userId) await invoke("set_user_id_for_backend", { userId });
    }
  } catch (e) {
    console.warn("Failed to provide user id to backend:", e);
  }
}

// == current (opened) entry == //
// `current` is a working copy of a history entry; edits are written back with saveCurrent()
const current = ref<SmartHistoryEntry | null>(null);
const currentEmail = computed(() =>
  current.value?.type === "email" ? (current.value as EmailHistoryEntry) : null,
);
const currentRoute = computed(() =>
  current.value?.type === "route" ? (current.value as RouteHistoryEntry) : null,
);
const newType = ref<"email" | "route">("email");
const confirmClear = ref(false);

function saveCurrent() {
  if (current.value) upsert(current.value, false);
}

function startNew(type?: "email" | "route") {
  if (type) newType.value = type;
  current.value = null;
  resetDetailState();
}

function resetDetailState() {
  eventTitle.value = "";
  departureReminder.value = false;
  conflictWarning.value = "";
  addEventError.value = "";
  addEventInfo.value = "";
  copied.value = false;
  mailInfo.value = "";
  routeError.value = "";
  transitSteps.value = [];
}

async function openEntry(entry: SmartHistoryEntry) {
  current.value = JSON.parse(JSON.stringify(entry));
  resetDetailState();
  eventTitle.value =
    entry.type === "email"
      ? entry.email.subject || entry.email.topic
      : `Trip to ${entry.route.destination}`;
  departureReminder.value =
    entry.type === "route" && !!entry.route.durationSeconds;
  if (!current.value!.eventTime) current.value!.eventTime = nextHourLocal();
  if (entry.type === "route") {
    await nextTick();
    await showRoute();
  }
}

function removeEntry(id: string) {
  remove(id);
  if (current.value?.id === id) startNew();
}

function clearHistory() {
  if (!confirmClear.value) {
    confirmClear.value = true;
    setTimeout(() => (confirmClear.value = false), 3000);
    return;
  }
  confirmClear.value = false;
  clear();
  startNew();
}

// == Email == //
const emailTopic = ref("");
const emailParticipants = ref<string[]>([]);
const participantInput = ref("");
const emailTime = ref(nextHourLocal());
const emailFormError = ref("");
const emailLoading = ref(false);
const copied = ref(false);
const mailInfo = ref("");
const emailRegex = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;

function addParticipant() {
  const raw = participantInput.value
    .split(/[,;\s]+/)
    .map((s) => s.trim())
    .filter(Boolean);
  if (!raw.length) return;
  const invalid = raw.filter((e) => !emailRegex.test(e));
  for (const e of raw) {
    if (emailRegex.test(e) && !emailParticipants.value.includes(e)) {
      emailParticipants.value.push(e);
    }
  }
  if (invalid.length) {
    emailFormError.value = `Invalid email: ${invalid.join(", ")}`;
    participantInput.value = invalid.join(", ");
  } else {
    emailFormError.value = "";
    participantInput.value = "";
  }
}

function removeParticipant(email: string) {
  emailParticipants.value = emailParticipants.value.filter((e) => e !== email);
}

async function generateEmail() {
  if (participantInput.value.trim()) addParticipant();
  emailFormError.value = "";
  if (!emailParticipants.value.length) {
    emailFormError.value = "Add at least one recipient.";
    return;
  }
  const topic = emailTopic.value.trim();
  if (!topic) {
    emailFormError.value = "Describe what the email should be about.";
    return;
  }
  if (!emailTime.value || isNaN(new Date(emailTime.value).getTime())) {
    emailTime.value = nextHourLocal();
  }

  emailLoading.value = true;
  try {
    const participants = [...emailParticipants.value];
    const contextEvent = buildCalendarEvent(topic, emailTime.value, participants);
    const response = await invoke<string>("generate_ai_email", {
      eventJson: JSON.stringify(contextEvent),
      emailTopic: topic,
      participants,
    });
    const parsed = JSON.parse(response);
    if (
      typeof parsed.response_text === "string" &&
      parsed.response_text.startsWith("🚫")
    ) {
      emailFormError.value = parsed.response_text;
      return;
    }
    const entry: EmailHistoryEntry = {
      id: crypto.randomUUID(),
      type: "email",
      createdAt: new Date().toISOString(),
      title: parsed.email_subject || topic,
      eventTime: emailTime.value,
      addedEventId: null,
      sourceEventId: null,
      email: {
        topic,
        subject: parsed.email_subject || "",
        body: parsed.response_text || "",
        participants,
      },
    };
    upsert(entry);
    emailTopic.value = "";
    emailParticipants.value = [];
    emailTime.value = nextHourLocal();
    await openEntry(entry);
  } catch (err) {
    console.error(err);
    emailFormError.value = errMessage(err, "Failed to generate email.");
  } finally {
    emailLoading.value = false;
  }
}

async function copyEmail() {
  if (!currentEmail.value) return;
  const { subject, body } = currentEmail.value.email;
  await navigator.clipboard.writeText(
    subject ? `Subject: ${subject}\n\n${body}` : body,
  );
  copied.value = true;
  setTimeout(() => (copied.value = false), 2000);
}

function openMailClient() {
  if (!currentEmail.value) return;
  const { participants, subject, body } = currentEmail.value.email;
  if (/android/i.test(navigator.userAgent)) {
    mailInfo.value = "Not supported on this platform yet — use copy instead.";
    return;
  }
  const mailto =
    `mailto:${participants.join(",")}` +
    `?subject=${encodeURIComponent(subject)}` +
    `&body=${encodeURIComponent(body)}`;
  window.open(mailto, "_blank");
  mailInfo.value = "Email client opened!";
}

// == Route == //
const routeOrigin = ref("");
const routeDestination = ref("");
const routeMode = ref<TravelMode>("car");
const routeTime = ref(nextHourLocal());
const routeFormError = ref("");
const routeLoading = ref(false);
const routeError = ref("");
const transitSteps = ref<google.maps.DirectionsStep[]>([]);
const mapEl = ref<HTMLElement | null>(null);

let map: google.maps.Map | null = null;
let directionsRenderer: google.maps.DirectionsRenderer | null = null;

// Show a clear message instead of Google's generic "can't load map" page
const stopAuthListener = onMapsAuthFailure(() => {
  routeError.value = MAPS_AUTH_FAILED_MSG;
  routeFormError.value = MAPS_AUTH_FAILED_MSG;
});
onBeforeUnmount(stopAuthListener);

function resolveOrigin(input: string): { origin: string; label: string } | null {
  const trimmed = input.trim();
  if (trimmed) return { origin: trimmed, label: trimmed };
  if (props.coordinates) {
    return {
      origin: `${props.coordinates.lat},${props.coordinates.lng}`,
      label: "Current location",
    };
  }
  return null;
}

async function computeRoute(
  origin: string,
  destination: string,
  mode: TravelMode,
  arriveBy: string | null,
): Promise<google.maps.DirectionsResult> {
  await loadGoogleMapsApi();
  const service = new google.maps.DirectionsService();
  const travelMode =
    mode === "car"
      ? google.maps.TravelMode.DRIVING
      : google.maps.TravelMode.TRANSIT;
  const arrival = arriveBy ? new Date(arriveBy) : null;
  const useArrival =
    mode === "transit" && arrival && arrival.getTime() > Date.now();
  return new Promise((resolve, reject) => {
    service.route(
      {
        origin,
        destination,
        travelMode,
        provideRouteAlternatives: false,
        ...(useArrival && { transitOptions: { arrivalTime: arrival! } }),
      },
      (result, status) => {
        if (status === "OK" && result) resolve(result);
        else
          reject(
            status === "NOT_FOUND" || status === "ZERO_RESULTS"
              ? "No route found between these locations."
              : status === "REQUEST_DENIED"
                ? MAPS_AUTH_FAILED_MSG
                : `Route request failed (${status}).`,
          );
      },
    );
  });
}

function applyRouteResult(
  entry: RouteHistoryEntry,
  result: google.maps.DirectionsResult,
) {
  const leg = result.routes[0]?.legs?.[0];
  entry.route.durationSeconds = leg?.duration?.value ?? null;
  entry.route.durationText = leg?.duration?.text ?? null;
  entry.route.distanceText = leg?.distance?.text ?? null;
}

function drawRoute(result: google.maps.DirectionsResult) {
  if (!mapEl.value) return;
  if (!map || map.getDiv() !== mapEl.value) {
    map = new google.maps.Map(mapEl.value, {
      zoom: 13,
      center: { lat: 51.5033, lng: -0.1195 },
      mapTypeControl: false,
      streetViewControl: false,
      fullscreenControl: false,
      zoomControl: false,
    });
    directionsRenderer = new google.maps.DirectionsRenderer({
      map,
      polylineOptions: { strokeColor: "#0078d7", strokeWeight: 5 },
    });
  }
  directionsRenderer!.setDirections(result);
  const steps = result.routes[0]?.legs?.[0]?.steps ?? [];
  transitSteps.value = currentRoute.value?.route.mode === "transit" ? steps : [];
}

// (Re)compute and draw the route for the opened entry
async function showRoute() {
  const entry = currentRoute.value;
  if (!entry) return;
  routeError.value = "";
  try {
    const result = await computeRoute(
      entry.route.origin,
      entry.route.destination,
      entry.route.mode,
      entry.eventTime,
    );
    if (current.value?.id !== entry.id) return; // user moved on
    applyRouteResult(entry, result);
    saveCurrent();
    await nextTick();
    drawRoute(result);
  } catch (err) {
    routeError.value = errMessage(err, "Failed to load route.");
  }
}

async function changeRouteMode(mode: TravelMode) {
  if (!currentRoute.value || currentRoute.value.route.mode === mode) return;
  currentRoute.value.route.mode = mode;
  transitSteps.value = [];
  await showRoute();
}

async function planRoute() {
  routeFormError.value = "";
  const destination = routeDestination.value.trim();
  if (!destination) {
    routeFormError.value = "Enter where you are going.";
    return;
  }
  const origin = resolveOrigin(routeOrigin.value);
  if (!origin) {
    routeFormError.value =
      "Enter a start location, or enable your location in Settings.";
    return;
  }
  routeLoading.value = true;
  try {
    // Validate the route first so failed lookups don't clutter history
    const result = await computeRoute(
      origin.origin,
      destination,
      routeMode.value,
      routeTime.value,
    );
    const entry: RouteHistoryEntry = {
      id: crypto.randomUUID(),
      type: "route",
      createdAt: new Date().toISOString(),
      title: destination,
      eventTime: routeTime.value || nextHourLocal(),
      addedEventId: null,
      sourceEventId: null,
      route: {
        origin: origin.origin,
        originLabel: origin.label,
        destination,
        mode: routeMode.value,
        durationSeconds: null,
        durationText: null,
        distanceText: null,
      },
    };
    applyRouteResult(entry, result);
    upsert(entry);
    routeOrigin.value = "";
    routeDestination.value = "";
    routeTime.value = nextHourLocal();
    current.value = JSON.parse(JSON.stringify(entry));
    resetDetailState();
    eventTitle.value = `Trip to ${destination}`;
    departureReminder.value = !!entry.route.durationSeconds;
    await nextTick();
    drawRoute(result);
  } catch (err) {
    routeFormError.value = errMessage(err, "Failed to plan route.");
  } finally {
    routeLoading.value = false;
  }
}

// == Add as event == //
const eventTitle = ref("");
const departureReminder = ref(false);
const addingEvent = ref(false);
const addEventError = ref("");
const addEventInfo = ref("");
const conflictWarning = ref("");

// Changing time/title invalidates a previous conflict warning
watch([eventTitle, () => current.value?.eventTime], () => {
  conflictWarning.value = "";
});

async function findConflict(date: Date): Promise<string | null> {
  try {
    const eventsJson = await invoke<string[]>("get_events");
    for (const raw of eventsJson) {
      const ev = JSON.parse(raw);
      if (ev.deleted) continue;
      const t = new Date(ev.time);
      if (
        t.getFullYear() === date.getFullYear() &&
        t.getMonth() === date.getMonth() &&
        t.getDate() === date.getDate() &&
        t.getHours() === date.getHours()
      ) {
        return ev.description || "another event";
      }
    }
  } catch (e) {
    console.warn("Conflict check failed:", e);
  }
  return null;
}

async function addAsEvent() {
  const entry = current.value;
  if (!entry) return;
  addEventError.value = "";
  addEventInfo.value = "";
  const title = eventTitle.value.trim() || entry.title;
  if (!entry.eventTime || isNaN(new Date(entry.eventTime).getTime())) {
    addEventError.value = "Pick a date and time for the event.";
    return;
  }
  const date = new Date(entry.eventTime);
  if (date.getTime() < Date.now()) {
    addEventError.value = "That time is in the past — pick a future time.";
    return;
  }

  addingEvent.value = true;
  try {
    if (!conflictWarning.value) {
      const conflict = await findConflict(date);
      if (conflict) {
        conflictWarning.value = `You already have "${conflict}" at that hour.`;
        return;
      }
    }
    conflictWarning.value = "";

    const participants = entry.type === "email" ? entry.email.participants : [];
    const calendarEvent = buildCalendarEvent(
      title,
      entry.eventTime,
      participants,
    );
    await provideMobileUserId();
    await invoke("save_event", { event: JSON.stringify(calendarEvent) });
    invoke("trigger_sync").catch((e) => console.warn("Sync failed:", e));

    entry.addedEventId = calendarEvent.id;
    saveCurrent();
    await tauriEmit("event-saved");

    if (
      entry.type === "route" &&
      departureReminder.value &&
      entry.route.durationSeconds
    ) {
      try {
        await invoke("schedule_smart_departure_notification", {
          eventJson: JSON.stringify(calendarEvent),
          travelSeconds: Math.round(entry.route.durationSeconds),
          destination: entry.route.destination,
        });
        addEventInfo.value = "Departure reminder scheduled.";
      } catch (err) {
        addEventInfo.value = "";
        addEventError.value = `Event added, but the reminder could not be set: ${errMessage(err, "unknown error")}`;
      }
    }
  } catch (err) {
    console.error(err);
    addEventError.value = errMessage(err, "Failed to add event.");
  } finally {
    addingEvent.value = false;
  }
}
</script>

<style scoped>
#smart-hub {
  display: grid;
  /* history column grows a little with the window; workspace takes the rest */
  grid-template-columns: clamp(14rem, 22vw, 20rem) minmax(0, 1fr);
  gap: clamp(0.75rem, 2vw, 1.5rem);
  /* main-content top padding (2rem) + section padding (2 x 1rem) */
  height: calc(100vh - 4.5rem);
  /* stretch with the window, keeping a responsive gap from both edges
     (same as the AI assistant chat) */
  width: calc(100% - 2 * clamp(0.5rem, 4vw, 4rem));
  margin: 0 auto;
  overflow: hidden;
}

#smart-hub.is-mobile {
  width: 100%;
  grid-template-columns: 1fr;
  grid-template-rows: auto auto;
  height: auto;
  overflow: visible;
  padding: 1rem 1rem 7rem;
}

/* history */
.hub-history {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
  background: var(--color-shadow);
  border-radius: 10px;
  padding: 1rem;
  overflow-y: auto;
  min-height: 0;
}

.hub-history-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.hub-history-header h2 {
  font-size: 1.1rem;
  color: var(--color-theme);
  font-weight: bold;
}

.hub-history-sub {
  display: flex;
  justify-content: space-between;
  align-items: center;
  font-size: 0.8rem;
  opacity: 0.8;
  border-bottom: 1px solid var(--color-theme);
  padding-bottom: 0.4rem;
}

.empty-hint {
  font-size: 0.85rem;
  opacity: 0.7;
}

.history-list {
  list-style: none;
  padding: 0;
  margin: 0;
  display: flex;
  flex-direction: column;
  gap: 0.4rem;
}

.history-list li {
  display: flex;
  align-items: flex-start;
  gap: 0.5rem;
  padding: 0.5rem;
  border-radius: 8px;
  border: 1px solid transparent;
  cursor: pointer;
}

.history-list li:hover {
  border-color: var(--color-theme);
}

.history-list li.active {
  border-color: var(--color-theme);
  background: var(--color-main);
}

.history-icon {
  flex-shrink: 0;
  padding-top: 2px;
}

.history-text {
  display: flex;
  flex-direction: column;
  min-width: 0;
  flex: 1;
}

.history-title {
  font-size: 0.9rem;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.history-meta {
  font-size: 0.72rem;
  opacity: 0.7;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.in-calendar {
  color: var(--color-theme);
  opacity: 1;
}

.history-delete {
  opacity: 0;
}

.history-list li:hover .history-delete,
.is-mobile .history-delete {
  opacity: 0.8;
}

/* workspace */
.hub-workspace {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 1rem;
  overflow-y: auto;
  padding: 0 0.25rem 2rem;
  min-height: 0;
  min-width: 0;
}

.tabs {
  display: flex;
  justify-content: center;
  gap: 1rem;
  width: 100%;
  padding: 1rem;
  border-bottom: 1px solid var(--color-theme);
}

.tabs button,
.mode-row button {
  padding: 0.5rem 1rem;
  border: none;
  background: var(--color-shadow);
  color: var(--color-text);
  cursor: pointer;
  border-radius: 6px;
}

.tabs button.active,
.mode-row button.active {
  background: var(--color-theme);
  color: var(--color-main);
}

.hub-form,
.hub-detail,
.add-event {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
  width: 100%;
}

.add-event {
  border-top: 1px solid var(--color-theme);
  padding-top: 1rem;
}

.hub-form h3,
.add-event h4 {
  color: var(--color-theme);
  font-weight: bold;
}

label {
  font-size: 0.85rem;
  opacity: 0.85;
}

input,
textarea {
  padding: 0.5rem;
  border: 1px solid var(--color-theme);
  border-radius: 4px;
  background-color: var(--color-shadow);
  color: var(--color-text);
  font-family: inherit;
}

input[type="datetime-local"] {
  color-scheme: light dark;
}

textarea {
  resize: vertical;
}

.row {
  display: flex;
  gap: 0.5rem;
  align-items: center;
  flex-wrap: wrap;
}

.row input:not([type="datetime-local"]):not([type="checkbox"]),
.grow {
  flex: 1;
  min-width: 10rem;
}

.chips {
  display: flex;
  flex-wrap: wrap;
  gap: 0.3rem;
}

.chip {
  display: inline-flex;
  align-items: center;
  gap: 0.3rem;
  padding: 0.2rem 0.6rem;
  border-radius: 999px;
  background: var(--color-shadow);
  border: 1px solid var(--color-theme);
  font-size: 0.8rem;
}

.chip button {
  background: none;
  border: none;
  color: var(--color-text);
  cursor: pointer;
  font-size: 1rem;
  line-height: 1;
}

.primary-btn {
  padding: 0.5rem 1rem;
  border: none;
  background: var(--color-theme);
  color: var(--color-main);
  cursor: pointer;
  border-radius: 4px;
  align-self: flex-start;
}

.primary-btn:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.secondary-btn {
  padding: 0.5rem 1rem;
  border: 1px solid var(--color-theme);
  background: transparent;
  color: var(--color-text);
  cursor: pointer;
  border-radius: 4px;
}

.link-btn {
  background: none;
  border: none;
  color: var(--color-theme);
  cursor: pointer;
  font-size: 0.8rem;
  padding: 0;
}

.link-btn:hover {
  text-decoration: underline;
}

.icon-btn {
  background: transparent;
  border: none;
  cursor: pointer;
  padding: 0.2rem;
  display: flex;
}

.detail-header {
  padding-top: 1rem;
}

.detail-field {
  display: flex;
  flex-direction: column;
  gap: 0.3rem;
}

.body-field {
  position: relative;
}

.copy-btn {
  position: absolute;
  top: 1.6rem;
  right: 0.4rem;
  z-index: 1;
}

.success {
  color: var(--color-success, #27ae60);
  font-size: 0.85rem;
}

.checkbox-label {
  display: flex;
  align-items: center;
  gap: 0.4rem;
  cursor: pointer;
}

.checkbox-label input {
  accent-color: var(--color-theme);
}

.route-summary {
  line-height: 1.5;
}

.hub-map {
  width: 100%;
  /* grows with the window height too */
  height: clamp(280px, 45vh, 640px);
  border-radius: 8px;
  border: 2px solid var(--color-theme);
}

.transit-steps {
  font-size: 0.9rem;
}

.transit-steps h4 {
  color: var(--color-theme);
  margin-bottom: 0.3rem;
}

.transit-steps ol {
  padding-left: 1.2rem;
  display: flex;
  flex-direction: column;
  gap: 0.3rem;
}

.steps-color {
  color: var(--color-theme);
}
</style>
