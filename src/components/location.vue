<template>
  <div class="location-container">
    <div class="permission-toggle">
      <label class="toggle-switch">
        <input 
          type="checkbox" 
          v-model="useUserLocation"
          @change="handleLocationToggle"
        />
        <span class="slider"></span>
      </label>
      <span class="toggle-label">Use my location</span>
      <div v-if="errorMessage" class="error">
        {{ errorMessage }}
      </div>
    </div>

    <div class="city-selector" v-if="!useUserLocation">
      <label for="city-select">Select City:</label>
      <select 
        id="city-select" 
        v-model="selectedCity"
        @change="handleCityChange"
      >
        <option value="">Choose a city...</option>
        <option 
          v-for="city in cities" 
          :key="city.name"
          :value="city"
        >
          {{ city.name }}
        </option>
      </select>
    </div>
  </div>
</template>

<script>
export default {
  name: 'LocationComponent',
  data() {
    return {
      useUserLocation: false,
      selectedCity: '',
      currentCoordinates: null,
      currentLocationName: '',
      errorMessage: '',
      cities: [
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
    }
  },
  mounted() {
  // Restore location preference
  const saved = localStorage.getItem('useUserLocation');
  if (saved !== null) {
    this.useUserLocation = saved === 'true';
    if (this.useUserLocation) {
      this.getUserLocation();
    }
  }
},
watch: {
  useUserLocation(newVal) {
    localStorage.setItem('useUserLocation', newVal);
  }
},
  methods: {
    handleLocationToggle() {
      this.clearError();
      if (this.useUserLocation) {
        this.getUserLocation();
      } else {
        this.currentCoordinates = null;
        this.currentLocationName = '';
        this.emitCoordinates();
      }
    },
    getUserLocation() {
      this.showError('Getting location...');
      
      // Use IP-based location
      fetch('http://ip-api.com/json/')
        .then(response => response.json())
        .then(data => {
          if (data.status === 'success') {
            this.currentCoordinates = {
              lat: data.lat,
              lng: data.lon
            };
            this.currentLocationName = `${data.city}, ${data.country}`;
            this.clearError();
            this.emitCoordinates();
          } else {
            throw new Error('Failed to get location from IP');
          }
        })
        .catch(error => {
          console.error('IP-based location error:', error);
          this.useUserLocation = false;
          this.showError('Unable to get your location. Please select a city manually.');
        });
    },
    handleCityChange() {
      this.clearError();
      if (this.selectedCity) {
        this.currentCoordinates = {
          lat: this.selectedCity.lat,
          lng: this.selectedCity.lng
        };
        this.currentLocationName = this.selectedCity.name;
        this.emitCoordinates();
      }
    },
    emitCoordinates() {
      this.$emit('updateCoordinates', this.currentCoordinates);
      this.$emit('updateLocationName', this.currentLocationName);
    },
    showError(message) {
      this.errorMessage = message;
      setTimeout(() => {
        this.clearError();
      }, 5000); // Clear error after 5 seconds
    },
    clearError() {
      this.errorMessage = '';
    }
  }
}
</script>

<style scoped>
.location-container {
  padding: 20px;
  border: 1px solid #ddd;
  border-radius: 8px;
  margin: 10px 0;
}

.permission-toggle {
  display: flex;
  align-items: center;
  margin-bottom: 15px;
  flex-wrap: wrap;
  gap: 10px;
}

.toggle-switch {
  position: relative;
  display: inline-block;
  width: 50px;
  height: 24px;
}

.toggle-switch input {
  opacity: 0;
  width: 0;
  height: 0;
}

.slider {
  position: absolute;
  cursor: pointer;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background-color: #ccc;
  transition: .4s;
  border-radius: 24px;
}

.slider:before {
  position: absolute;
  content: "";
  height: 18px;
  width: 18px;
  left: 3px;
  bottom: 3px;
  background-color: white;
  transition: .4s;
  border-radius: 50%;
}

input:checked + .slider {
  background-color: #2196F3;
}

input:checked + .slider:before {
  transform: translateX(26px);
}

.toggle-label {
  font-weight: 500;
}

.city-selector {
  margin-bottom: 15px;
}

.city-selector label {
  display: block;
  margin-bottom: 5px;
  font-weight: 500;
}

.city-selector select {
  width: 100%;
  padding: 8px;
  border: 1px solid #ccc;
  border-radius: 4px;
  font-size: 14px;
}

.location-info {
  background-color: #f5f5f5;
  padding: 10px;
  border-radius: 4px;
}

.location-info p {
  margin: 0 0 5px 0;
  font-weight: 500;
}

.location-info small {
  color: #666;
}
</style>

