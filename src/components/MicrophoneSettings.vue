<template>
  <h2>Microphone:</h2>
  <section class="container">
    <section class="mic-status">
      <span class="status-dot" :class="permission"></span>
      <span>{{ statusText }}</span>
      <button
        v-if="permission !== 'granted'"
        class="allow-btn"
        :disabled="busy || permission === 'unsupported'"
        @click="requestAccess"
      >
        {{ busy ? "Requesting…" : "Allow microphone access" }}
      </button>
    </section>

    <section class="selector">
      <label for="mic-select">Input device:</label>
      <div class="mic-row">
        <select
          id="mic-select"
          :value="selectedDeviceId"
          @change="selectDevice(($event.target as HTMLSelectElement).value)"
        >
          <option value="">System default</option>
          <option v-for="(mic, i) in devices" :key="mic.deviceId" :value="mic.deviceId">
            {{ micLabel(mic, i) }}
          </option>
        </select>
        <button class="refresh-btn" @click="refreshDevices" title="Refresh device list">⟳</button>
      </div>
      <small v-if="permission !== 'granted'" class="hint">
        Allow access to see the names of your microphones.
      </small>
    </section>

    <div v-if="error" class="error">{{ error }}</div>
  </section>
</template>

<script setup lang="ts">
import { computed, onMounted } from "vue";
import { useMicrophone, micLabel } from "../composables/useMicrophone";

const {
  devices,
  selectedDeviceId,
  permission,
  error,
  busy,
  init,
  requestAccess,
  refreshDevices,
  selectDevice,
} = useMicrophone();

const statusText = computed(() => {
  switch (permission.value) {
    case "granted":
      return "Access allowed";
    case "denied":
      return "Access denied";
    case "unsupported":
      return "Not available";
    default:
      return "Access not granted yet";
  }
});

onMounted(() => {
  init();
});
</script>

<style scoped src="@/assets/checkbox-slider.css"></style>
<style scoped>
.mic-status {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  flex-wrap: wrap;
  margin-bottom: 15px;
}

.status-dot {
  width: 10px;
  height: 10px;
  border-radius: 50%;
  background: var(--color-text);
  opacity: 0.5;
}

.status-dot.granted {
  background: var(--color-success, #27ae60);
  opacity: 1;
}

.status-dot.denied,
.status-dot.unsupported {
  background: var(--color-error, #e74c3c);
  opacity: 1;
}

.allow-btn {
  margin-left: auto;
  padding: 0.4rem 0.8rem;
  border: none;
  border-radius: 6px;
  background: var(--color-theme);
  color: var(--color-dark);
  cursor: pointer;
}

.allow-btn:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.mic-row {
  display: flex;
  gap: 0.5rem;
}

.mic-row select {
  flex: 1;
  min-width: 0;
  background-color: var(--color-shadow);
  color: var(--color-text);
}

.refresh-btn {
  padding: 0 0.7rem;
  border: 1px solid var(--color-text);
  border-radius: 4px;
  background: transparent;
  color: var(--color-text);
  cursor: pointer;
  font-size: 1rem;
}

.hint {
  display: block;
  margin-top: 5px;
  opacity: 0.7;
}
</style>
