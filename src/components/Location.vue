<template>
  <h2>location:</h2>
  <section class="container">
    <section class="permission-toggle">
      <label class="toggle-switch">
        <input type="checkbox" v-model="useUserLocation" @change="handleLocationToggle"/>
        <span class="slider"></span>
      </label>
      <span class="toggle-label">Use my location</span>
      <div v-if="errorMessage" class="error">
        {{ errorMessage }}
      </div>
    </section>

    <section class="selector" v-if="!useUserLocation">
      <label for="select">Select City:</label>
      <select id="select" v-model="selectedCity" @change="handleCityChange">
        <option :value="null">Choose a city...</option>
        <option v-for="city in cities" :key="city.name" :value="city">
          {{ city.name }}
        </option>
      </select>
    </section>
  </section>
</template>

<script setup lang="ts">
import { ref, watch, onMounted } from 'vue'

const emit = defineEmits(['updateCoordinates', 'updateLocationName'])

const useUserLocation = ref(false)
const selectedCity = ref<{ name: string; lat: number; lng: number } | null>(null)
const currentCoordinates = ref<{ lat: number; lng: number } | null>(null)
const currentLocationName = ref('')
const errorMessage = ref('')

const cities = [
  { name: 'London', lat: 51.5074, lng: -0.1278 },
  { name: 'New York', lat: 40.7128, lng: -74.0060 },
  { name: 'Paris', lat: 48.8566, lng: 2.3522 },
  { name: 'Tokyo', lat: 35.6762, lng: 139.6503 },
  { name: 'Sydney', lat: -33.8688, lng: 151.2093 },
  { name: 'Berlin', lat: 52.5200, lng: 13.4050 },
  { name: 'Madrid', lat: 40.4168, lng: -3.7038 },
  { name: 'Rome', lat: 41.9028, lng: 12.4964 },
  { name: 'Moscow', lat: 55.7558, lng: 37.6176 },
  { name: 'Dubai', lat: 25.2048, lng: 55.2708 }
]

function handleLocationToggle() {
  clearError()
  if (useUserLocation.value) {
    getUserLocation()
  } else {
    currentCoordinates.value = null
    currentLocationName.value = ''
    emitCoordinates()
  }
}

function getUserLocation() {
  showError('Getting location...')
  fetch('http://ip-api.com/json/')
    .then(response => response.json())
    .then(data => {
      if (data.status === 'success') {
        currentCoordinates.value = {
          lat: data.lat,
          lng: data.lon
        }
        currentLocationName.value = `${data.city}, ${data.country}`
        clearError()
        emitCoordinates()
      } else {
        throw new Error('Failed to get location from IP')
      }
    })
    .catch(error => {
      console.error('IP-based location error:', error)
      useUserLocation.value = false
      showError('Unable to get your location. Please select a city manually.')
    })
}

function handleCityChange() {
  clearError()
  if (selectedCity.value) {
    currentCoordinates.value = {
      lat: selectedCity.value.lat,
      lng: selectedCity.value.lng
    }
    currentLocationName.value = selectedCity.value.name
    emitCoordinates()
  }
}

function emitCoordinates() {
  emit('updateCoordinates', currentCoordinates.value)
  emit('updateLocationName', currentLocationName.value)
}

function showError(message: string) {
  errorMessage.value = message
  setTimeout(() => {
    clearError()
  }, 5000)
}

function clearError() {
  errorMessage.value = ''
}

// Restore location preference on mount
onMounted(() => {
  const saved = localStorage.getItem('useUserLocation')
  if (saved !== null) {
    useUserLocation.value = saved === 'true'
    if (useUserLocation.value) {
      getUserLocation()
    }
  }
})

// Watch for changes to useUserLocation and persist
watch(useUserLocation, (newVal) => {
  localStorage.setItem('useUserLocation', String(newVal))
})
</script>

<style scoped src="@/assets/checkbox-slider.css"></style>


