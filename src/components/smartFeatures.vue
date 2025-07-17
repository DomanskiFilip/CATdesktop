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
        <button @click="setMode('car')"><svg xmlns="http://www.w3.org/2000/svg" height="24px" viewBox="0 -960 960 960" width="24px" fill="var(--color-text)"><path d="M240-200v40q0 17-11.5 28.5T200-120h-40q-17 0-28.5-11.5T120-160v-320l84-240q6-18 21.5-29t34.5-11h440q19 0 34.5 11t21.5 29l84 240v320q0 17-11.5 28.5T800-120h-40q-17 0-28.5-11.5T720-160v-40H240Zm-8-360h496l-42-120H274l-42 120Zm-32 80v200-200Zm100 160q25 0 42.5-17.5T360-380q0-25-17.5-42.5T300-440q-25 0-42.5 17.5T240-380q0 25 17.5 42.5T300-320Zm360 0q25 0 42.5-17.5T720-380q0-25-17.5-42.5T660-440q-25 0-42.5 17.5T600-380q0 25 17.5 42.5T660-320Zm-460 40h560v-200H200v200Z"/></svg></button>
        <button @click="setMode('transit')"><svg xmlns="http://www.w3.org/2000/svg" height="24px" viewBox="0 -960 960 960" width="24px" fill="var(--color-text)"><path d="M240-120q-17 0-28.5-11.5T200-160v-82q-18-20-29-44.5T160-340v-380q0-83 77-121.5T480-880q172 0 246 37t74 123v380q0 29-11 53.5T760-242v82q0 17-11.5 28.5T720-120h-40q-17 0-28.5-11.5T640-160v-40H320v40q0 17-11.5 28.5T280-120h-40Zm242-640h224-448 224Zm158 280H240h480-80Zm-400-80h480v-120H240v120Zm100 240q25 0 42.5-17.5T400-380q0-25-17.5-42.5T340-440q-25 0-42.5 17.5T280-380q0 25 17.5 42.5T340-320Zm280 0q25 0 42.5-17.5T680-380q0-25-17.5-42.5T620-440q-25 0-42.5 17.5T560-380q0 25 17.5 42.5T620-320ZM258-760h448q-15-17-64.5-28.5T482-800q-107 0-156.5 12.5T258-760Zm62 480h320q33 0 56.5-23.5T720-360v-120H240v120q0 33 23.5 56.5T320-280Z"/></svg></button>
        <span id="google-map">
          <!-- Placeholder for Google Maps iframe -->
          <iframe v-show="mode === 'car'" id="google-map-car" src="https://www.google.com/maps/embed?pb=!1m18!1m12!1m3!1d3153.123456789012!2d-122.419415684681!3d37.774929279759!2m3!1f0!2f0!3f0!3m2!1i1024!2i768!4f13.1!3m3!1m2!1s0x8085809c8b8b8b8b%3A0x8b8b8b8b8b8b8b8b!2sSan%20Francisco%2C%20CA%2094105%2C%20USA!5e0!3m2!1sen!2sin!4v1616161616161" allowfullscreen="false" loading="lazy"></iframe>
          <iframe v-show="mode === 'transit'"  id="google-map-transit" src="https://www.google.com/maps/embed?pb=!1m18!1m12!1m3!1d3153.123456789012!2d-122.419415684681!3d37.774929279759!2m3!1f0!2f0!3f0!3m2!1i1024!2i768!4f13.1!3m3!1m2!1s0x8085809c8b8b8b8b%3A0x8b8b8b8b8b8b8b8b!2sSan%20Francisco%2C%20CA%2094105%2C%20USA!5e0!3m2!1sen!2sin!4v1616161616161" allowfullscreen="false" loading="lazy"></iframe>
        </span>
      </span>
    </div>
  </section>
</template>

<script setup lang="ts">
import { ref, onMounted, watch } from 'vue'

const props = defineProps<{ event: any }>()
const routeReady = ref(true)
const mode = ref('car')

// Placeholder for LLM integration and Google Maps API
const setMode = (newMode: string) => {
  mode.value = newMode
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
  border: none;
}
</style>