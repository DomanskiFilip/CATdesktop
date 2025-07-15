<template>
  <section class="smart-features-container">
    <button id="close" @click="$emit('close')"><svg xmlns="http://www.w3.org/2000/svg" height="24px" viewBox="0 -960 960 960" width="24px" fill="var(--color-text)"><path d="m256-200-56-56 224-224-224-224 56-56 224 224 224-224 56 56-224 224 224 224-56 56-224-224-224 224Z"/></svg></button>
    <h3>Smart Features for Event</h3>
    <div v-if="event">
      <h2>Description: {{ event.description }}</h2>
      <h2>Time: {{ event.time }}</h2>
      <!-- LLM and Google Maps integration will go here -->
      <!-- After LLM response, show Google Map with route -->
      <span v-if="routeReady">
        <span id="google-map"></span>
        <button @click="setMode('car')">Car Route</button>
        <button @click="setMode('transit')">Public Transit</button>
      </span>
    </div>
  </section>
</template>

<script setup lang="ts">
import { ref, onMounted, watch } from 'vue'

const props = defineProps<{ event: any }>()
const routeReady = ref(false)
const mode = ref('car')

// Placeholder for LLM integration and Google Maps API
const setMode = (newMode: string) => {
  mode.value = newMode
  // TODO: Update Google Maps route display
}

// TODO: After LLM response, set routeReady.value = true and show map
</script>

<style scoped>
.smart-features-container {
  width: 200%;
  margin: 0 auto;
  background: var(--color-shadow);
  border-radius: 10px;
  box-shadow: 0 4px 20px rgba(0,0,0,0.2);
  padding: 2rem;
  z-index: 2;
  display: flex;
  flex-direction: column;
  align-items: center;
}

#close {
  background: transparent;
  border: none;
  cursor: pointer;
  padding: 0.5rem;
  color: var(--color-text);
  margin-left: auto;
}

#google-map {
  width: 400px;
  height: 300px;
  margin-top: 1rem;
  border-radius: 8px;
  border: 1px solid #ccc;
}
</style>