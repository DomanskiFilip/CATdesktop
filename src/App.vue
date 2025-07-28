<template>
  <TitleBar v-if="!isMobile" />
  <!-- loading screen -->
  <Transition name="loading-shrink">
  <section v-if="isLoading" id="loading-screen">
    <div id="loader"></div>
    <span>Loading...</span>
  </section>
  </Transition>
    <!-- login/register page -->
  <section v-if="!loggedIn" id="login-register-page">
    <Login v-if="showLogin" @updateLoggedIn="loggedIn = $event"/>
    <Register v-else />
    <div>
      <span v-if="showLogin">
        do not have an account? &rarr; 
        <button @click="showLogin = false">
          Register
        </button>
      </span>
      <span v-else>
        already have an account? &rarr; 
        <button @click="showLogin = true">
          Login
        </button>
      </span>
    </div>
  </section>

  <!-- main page -->
  <section v-if="loggedIn && !isLoading" id="main-page" :class="{ 'is-mobile': isMobile }">
    <section id="side-bar">
      <button @click="moreInfo(ismoreInfoVisible)">
        <svg xmlns="http://www.w3.org/2000/svg" height="24px" viewBox="0 -960 960 960" width="20px" fill="var(--color-text)">
          <path d="M120-240v-80h720v80H120Zm0-200v-80h720v80H120Zm0-200v-80h720v80H120Z"/>
        </svg>
      </button>
      <button @click="async () => { changeSection('section1'); emit('event-saved'); /* emit event-saved to refresh calendar */ }" :class="{ active: activeSection === 'section1' }">
        <svg xmlns="http://www.w3.org/2000/svg" height="24px" viewBox="0 -960 960 960" width="24px" fill="var(--color-text)">
          <path d="M200-80q-33 0-56.5-23.5T120-160v-560q0-33 23.5-56.5T200-800h40v-80h80v80h320v-80h80v80h40q33 0 56.5 23.5T840-720v560q0 33-23.5 56.5T760-80H200Zm0-80h560v-400H200v400Zm0-480h560v-80H200v80Zm0 0v-80 80Zm80 240v-80h400v80H280Zm0 160v-80h280v80H280Z"/>
        </svg>
        <span class="moreInfo">CALENDAR</span>
      </button>
      <button @click="changeSection('section2')" :class="{ active: activeSection === 'section2' }">
        <svg width="25px" height="25px" viewBox="0 0 427 341"  stroke="var(--color-text)" stroke-width="20" xmlns="http://www.w3.org/2000/svg">
          <path d="M289.565 321.032C289.565 321.032 289.497 321.031 289.866 321.082C289.998 320.722 289.761 320.312 289.525 319.901C273.271 309.857 262.722 295.325 256.123 277.632C252.541 268.028 251.686 258.011 251.037 247.906C250.737 243.232 249.756 238.601 248.955 233.091C248.343 232.137 247.86 231.957 247.378 231.957C198.429 231.957 149.48 231.962 100.531 232.023C99.1617 232.024 97.793 232.623 96.4239 232.943C95.6161 237.441 94.6124 241.915 94.0706 246.445C93.7219 249.36 93.2695 251.923 89.7082 251.8C85.8441 251.666 86.1877 248.366 86.2426 245.97C86.6001 230.366 91.3388 216.091 99.6825 202.943C113.434 181.273 133.318 168.748 158.283 164.287C176.3 161.067 193.598 164.13 210.086 172.019C236.392 184.605 251.562 205.85 257.63 233.86C259.189 241.056 259.436 248.568 259.918 255.959C261.046 273.244 267.513 288.075 279.562 300.53C287.112 308.334 295.238 315.365 305.578 319.142C308.858 320.34 312.548 320.416 316.959 321.04C331.331 321.074 344.795 321.074 358.26 321.074C358.293 320.713 358.327 320.353 358.36 319.992C357.18 319.672 355.999 319.352 354.818 319.031C338.842 310.995 327.562 298.37 319.612 282.617C317.743 278.914 316.603 274.844 315.073 270.085C314.929 268.401 314.76 267.575 314.761 266.748C314.781 226.437 314.816 186.126 314.849 145.815C314.85 144.363 314.849 142.91 314.849 140.986C280.859 140.986 247.274 140.986 213.69 140.986C212.956 139.385 211.207 136.894 211.682 136.352C213.009 134.838 215.171 133.883 217.17 133.191C218.513 132.727 220.14 133.078 221.64 133.077C280.969 133.076 340.298 133.076 399.628 133.077C400.961 133.077 402.319 132.931 403.623 133.134C408.575 133.902 409.428 135.891 406.194 140.7C379.55 140.917 353.616 140.927 327.683 141.054C326.056 141.062 324.432 141.991 322.807 142.491C322.763 183.582 322.531 224.674 322.812 265.763C322.879 275.488 328.623 283.461 334.351 290.892C346.38 306.497 361.972 316.339 381.735 319.213C387.158 320.002 390.919 323.881 390.604 328.253C390.119 329.051 389.968 329.413 389.817 329.776C389.817 329.776 389.809 329.828 389.375 329.745C386.782 329.78 384.624 329.998 382.465 330.002C325.314 330.098 268.164 330.166 211.013 330.274C208.878 330.278 206.744 330.624 204.609 330.811C199.13 330.823 193.651 330.834 187.329 330.644C160.21 330.291 133.934 330.14 107.658 329.988C92.5577 324.543 78.0757 317.79 65.0337 308.379C52.388 299.254 41.9546 287.934 33.216 274.967C23.6469 260.768 17.0833 245.271 13.5176 228.662C11.4979 219.254 9.88891 209.441 10.2886 199.902C10.9637 183.793 13.4468 167.875 19.542 152.575C24.3491 140.509 30.5848 129.406 38.2924 119.148C47.8153 106.475 59.5455 96.2563 73.0922 87.732C98.3896 71.8136 125.968 66.1294 155.232 68.2111C178.314 69.8531 199.456 77.9605 218.764 90.746C219.578 91.285 220.386 91.8341 221.567 92.6275C226.067 83.5786 230.449 74.7817 234.818 65.9778C241.449 52.6124 248.069 39.2411 254.698 25.8743C256.714 21.8108 258.537 17.6311 260.864 13.7538C261.797 12.1993 264.171 10.1017 265.316 10.4064C267.119 10.886 269.334 12.9507 269.81 14.7567C272.31 24.2413 276.001 33.1551 281.926 40.9463C295.682 59.0341 313.643 69.6275 336.654 71.1465C362.466 72.8505 382.815 62.2522 399.387 43.3712C402.579 39.735 404.464 34.981 407.366 31.0482C408.562 29.4275 410.959 27.574 412.612 27.745C414.065 27.8955 415.529 30.5787 416.44 32.4154C417.05 33.644 416.768 35.3484 416.77 36.84C416.785 58.3383 416.801 79.8367 416.748 101.335C416.743 103.286 416.682 105.433 415.881 107.126C415.173 108.622 413.396 109.612 412.091 110.825C410.949 109.479 409.419 108.293 408.777 106.74C408.181 105.297 408.478 103.462 408.476 101.799C408.461 85.3005 408.466 68.8018 408.465 52.3031C408.465 50.5623 408.465 48.8214 408.465 45.8907C387.008 70.6951 361.774 83.5498 329.597 78.7253C298.035 73.9928 276.943 55.7527 264.278 25.1089C261.154 31.0396 258.648 35.5198 256.395 40.1239C247.79 57.7092 239.315 75.3584 230.654 92.9158C229.506 95.2428 227.791 97.4064 225.936 99.2381C223.61 101.535 220.88 102.141 217.788 100.079C206.163 92.3257 193.929 85.8856 180.48 81.699C164.325 76.6703 147.773 74.5197 131.113 76.1978C109.018 78.4236 88.4334 86.05 70.4942 99.2471C43.69 118.966 26.6039 145.173 20.9243 178.286C14.2961 216.931 23.348 251.435 48.6677 281.5C63.6293 299.266 82.1041 311.943 104.238 319.112C107.322 320.111 110.634 320.897 113.85 320.929C157.806 321.371 201.764 321.664 246.666 322.033C261.595 321.725 275.58 321.379 289.565 321.032ZM182.097 171.951C173.373 172.3 164.42 171.519 155.971 173.23C137.115 177.048 121.459 186.892 110.031 202.542C105.399 208.885 101.946 216.089 97.4482 223.768C148.124 223.768 197.075 223.768 246.006 223.768C246.006 223.164 246.102 222.811 245.984 222.559C245.782 222.127 245.321 221.798 245.183 221.358C240.675 206.982 231.338 195.853 219.669 187.178C208.942 179.203 196.687 173.548 182.097 171.951Z"/>
        </svg>
        <span class="moreInfo">ASSISTANT AI</span>
      </button>
      <button @click="changeSection('section3')" :class="{ active: activeSection === 'section3' }">
        <svg xmlns="http://www.w3.org/2000/svg" height="24px" viewBox="0 -960 960 960" width="24px" fill="var(--color-text)">
          <path d="m403-96-22-114q-23-9-44.5-21T296-259l-110 37-77-133 87-76q-2-12-3-24t-1-25q0-13 1-25t3-24l-87-76 77-133 110 37q19-16 40.5-28t44.5-21l22-114h154l22 114q23 9 44.5 21t40.5 28l110-37 77 133-87 76q2 12 3 24t1 25q0 13-1 25t-3 24l87 76-77 133-110-37q-19 16-40.5 28T579-210L557-96H403Zm59-72h36l19-99q38-7 71-26t57-48l96 32 18-30-76-67q6-17 9.5-35.5T696-480q0-20-3.5-38.5T683-554l76-67-18-30-96 32q-24-29-57-48t-71-26l-19-99h-36l-19 99q-38 7-71 26t-57 48l-96-32-18 30 76 67q-6 17-9.5 35.5T264-480q0 20 3.5 38.5T277-406l-76 67 18 30 96-32q24 29 57 48t71 26l19 99Zm18-168q60 0 102-42t42-102q0-60-42-102t-102-42q-60 0-102 42t-42 102q0 60 42 102t102 42Zm0-144Z"/>
        </svg>
        <span class="moreInfo">SETTINGS</span>
      </button>
      <button @click="logout()">
        <svg xmlns="http://www.w3.org/2000/svg" height="24px" viewBox="0 -960 960 960" width="24px" fill="var(--color-text)"><path d="M200-120q-33 0-56.5-23.5T120-200v-560q0-33 23.5-56.5T200-840h280v80H200v560h280v80H200Zm440-160-55-58 102-102H360v-80h327L585-622l55-58 200 200-200 200Z"/></svg>
        <span class="moreInfo">LOG OUT</span>
      </button>
    </section>
    <section id="main-content">
      <section v-show="activeSection === 'section1'" class="content-section">
        <div id="theme-background-element"></div> <!-- background visual element -->
        <Calendar :currentCoordinates="currentCoordinates" />
      </section>
      <section v-show="activeSection === 'section2'" class="content-section">
        <AiAssistant />
      </section>
      <section v-show="activeSection === 'section3'" class="content-section">
        <div id="theme-background-element"></div> <!-- background visual element -->
        <h2>SETTINGS:</h2>
        <hr>
        <Themes />
        <Notifications />
        <Location @updateCoordinates="handleLocationUpdate" @updateLocationName="handleLocationNameUpdate" />
        <Oauth />
      </section>
    </section>
  </section>

</template>

<script setup lang="ts">
import { ref, watch, onMounted, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen, emit } from '@tauri-apps/api/event'
import TitleBar from './components/TitleBar.vue'
import Oauth from './components/Oauth.vue'
import Location from './components/Location.vue'
import Login from './components/Login.vue'
import Register from './components/Register.vue'
import Themes from './components/Themes.vue'
import Calendar from './components/Calendar.vue'
import AiAssistant from './components/AiAssistant.vue'
import Notifications from './components/Notifications.vue'
import { platform } from '@tauri-apps/plugin-os'
import { retrieve as keystoreRetrieve } from '@impierce/tauri-plugin-keystore'

const isMobile = computed(() => /Android|webOS|iPhone|iPad|iPod|BlackBerry|IEMobile|Opera Mini/i.test(navigator.userAgent))
const isLoading = ref(true)
const showLogin = ref(true)
const loggedIn = ref(false)
const activeSection = ref('section1')
const ismoreInfoVisible = ref(false)
const currentCoordinates = ref<{ lat: number, lng: number } | null>(null)
const currentLocationName = ref('')

async function provideTokensToBackend() {
  const os = await platform();
  if (os === 'android' || os === 'ios') {
    try {
      // Retrieve the single JSON string from keystore
      const tokensJson = await keystoreRetrieve('default', 'default');
      if (tokensJson) {
        // Send the JSON string directly to the backend
        await invoke('set_tokens_for_autologin', { tokensJson: tokensJson });
      }
    } catch (e) {
      console.error("Failed to provide tokens to backend:", e);
    }
  }
}

// Function to log out the user
const logout = async () => {
  try {
    await invoke('logout_user');
    loggedIn.value = false;
  } catch (error) {
    console.error("Logout failed:", error);
  }
}

// Watch for changes in loggedIn and update the background element
watch(loggedIn, (newValue) => {
  const backgroundElement = document.getElementById('theme-background-element')
  if (backgroundElement) {
    if (newValue) {
      backgroundElement.classList.add('logged-in-background')
    } else {
      backgroundElement.classList.remove('logged-in-background')
    }
  }
})

// Function to change sections useing buttons in sidebar
const changeSection = (sectionId: string) => {
  activeSection.value = sectionId
}

// Function to toggle the visibility of the more info section
const moreInfo = (isVisible: boolean) => {
  ismoreInfoVisible.value = !isVisible;
  const moreInfos = document.querySelectorAll('.moreInfo')
  const sideBar = document.getElementById('side-bar')
  const mainContent = document.getElementById('main-content')
  moreInfos.forEach((element) => {
    if (ismoreInfoVisible.value) {
      element.classList.add('active')
    } else {
      element.classList.remove('active')
    }
  })
  if (sideBar && mainContent) {
    if (ismoreInfoVisible.value) {
      sideBar.style.width = '8.5rem'
      mainContent.style.marginLeft = '8.5rem'
    } else {
      sideBar.style.width = '3rem'
      mainContent.style.marginLeft = '3rem'
    }
  }
}

// Function to handle location updates from the location component
const handleLocationUpdate = async (coordinates: { lat: number, lng: number } | null) => {
  currentCoordinates.value = coordinates
  if (coordinates) {
    // Emit location change event to notify calendar component
    emit('location-changed', { 
      latitude: coordinates.lat, 
      longitude: coordinates.lng 
    })
    await invoke('set_user_coordinates', {
      latitude: coordinates.lat,
      longitude: coordinates.lng
    });
  }
}

const handleLocationNameUpdate = (name: string) => {
  currentLocationName.value = name
}

onMounted(async () => {
  // Listen for backend auto-login events
  await listen('auto-login-completed', (event) => {
    const loginResult = event.payload as boolean;
    loggedIn.value = loginResult;
    isLoading.value = false;
    if (!loginResult) {
      console.log('Auto-login failed - tokens may be expired or invalid');
    }
  });
  const os = await platform()
  if (os === 'android' || os === 'ios') {
    await provideTokensToBackend();
  }
  // Fallback: Check current login status if event was missed
  setTimeout(async () => {
    if (!loggedIn.value) {
      try {
        const isLoggedIn = await invoke('check_login_status') as boolean;
        loggedIn.value = isLoggedIn;
      } catch (error) {
        console.error("Login status check failed:", error);
        loggedIn.value = false;
      }
    }
    isLoading.value = false; 
  }, 5000); // Give auto-login time to complete
});
</script>

<style scoped>
/* loading styles */
#loading-screen {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: 100vh;
  width: 100vw;
  background: var(--color-main);
  z-index: 9999;
  position: fixed;
  top: 0;
  left: 0;
}

.loading-shrink-enter-active,
.loading-shrink-leave-active {
  transition: transform 1s cubic-bezier(.68,-0.55,.27,1.55), opacity 0.5s;
}
.loading-shrink-enter-from,
.loading-shrink-leave-to {
  transform: scale(0.8);
  opacity: 0;
}
.loading-shrink-enter-to,
.loading-shrink-leave-from {
  transform: scale(1);
  opacity: 1;
}

/* main page styles */
#main-page {
  display: flex;
  flex-direction: row;
  height: 100%;
  width: 100%;
  background-color: var(--color-shadow);
}

.hidden {
  display: none;
}

/* login page styles */
#login-register-page {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: 100%;
  width: 100%;
  gap: 1rem;
}

#login-register-page button {
  background-color: transparent;
  border: none;
  color: var(--color-theme);
  cursor: pointer;
  font-size: 1rem;
}

#login-register-page button:hover {
  text-decoration: underline;
}

#theme-background-element.logged-in-background {
  height: 40vh;
  width: 30vw;
}

/* side bar styles */
#side-bar {
  position: fixed;
  top: 0;
  left: 0;
  width: 3rem;
  height: 100%;
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  justify-content: flex-start;
  padding: 0.5rem;
  padding-top: 2.5rem;
  gap: 0.5rem;
  transition: transform 0.2s ease, width 0.4s ease;
  background-color: var(--color-shadow);
}

#side-bar button {
  background-color: transparent;
  border: 1px solid var(--color-border);
  border-radius: 10px;
  cursor: pointer;
  width: 2rem;
  height: 2rem;
  display: flex;
  align-items: center;
  justify-content: center;
  position: relative;
  transition: transform 0.2s ease;
}

#side-bar button:active {
  transform: scale(1.1);
}

#side-bar button::before {
  content: '';
  position: absolute;
  left: -8px;
  width: 5px;
  height: 70%;
  background-color: var(--color-theme);
  border-radius: 10px;
  opacity: 0;
}

#side-bar button.active::before {
  opacity: 1;
}

#side-bar button:first-of-type {
  border: none;
}

#side-bar button:first-of-type::before {
  display: none;
}

#side-bar button:nth-last-of-type(2) {
  margin-top: auto;
}

#side-bar button:hover {
background-color: var(--color-theme);
border-color: var(--color-theme);
}

#side-bar button:hover svg {
fill: var(--color-dark);
stroke: var(--color-dark); 
}

.moreInfo {
  position: absolute;
  top: 0;
  left: 100%;
  color: var(--color-text);
  padding: 0.5rem;
  border-radius: 5px;
  opacity: 0;
  clip-path: inset(0 100% 0 0);
  transition: clip-path 0.4s ease, opacity 0.4s ease;
  text-wrap: nowrap;
  font-size: 0.75rem;
}

.moreInfo.active {
  opacity: 1;
  clip-path: inset(0 0 0 0);
}

#side-bar button:hover .moreInfo{
  color: var(--color-theme);
}

.moreInfo:hover {
  color: var(--color-theme);
}

/* main content styles */
#main-content {
  margin-left: 3.5rem;
  padding-top: 2rem;
  height: 100%;
  width: 100%;
  overflow-y: hidden;
  transition: transform 0.2s ease, margin-left 0.4s ease;
  overflow-x: hidden;
}

#main-content .content-section {
 border-top-left-radius: 10px;
 height: 100%;
 background-color: var(--color-main);
 padding: 1rem;
}

#theme-background-element {
  position: absolute;
  bottom: 0;
  right: 0;
  width: 42vw;
  height: 50vh;
  opacity: 30%;
  background-color: var(--color-theme);
  clip-path: polygon(100% 100%, 0 100%, 100% 0);
  pointer-events: none;
}

/* Mobile */
#main-page.is-mobile {
  flex-direction: column;
  width: 100vw;
  height: 100vh;
}

#main-page.is-mobile * {
  touch-action: pan-y;
}

#main-page.is-mobile #side-bar {
  position: fixed;
  top: auto;
  bottom: 0;
  left: 0;
  right: 0;
  width: 100vw;
  height: 6rem;
  flex-direction: row;
  align-items: center;
  justify-content: center;
  padding-top: 0.5rem;
  padding-left: 0.5rem;
  border-top-left-radius: 10px;
  border-top-right-radius: 10px;
  border-bottom-left-radius: 0;
  border-bottom-right-radius: 0;
  z-index: 1001;
}

#main-page.is-mobile #side-bar button {
  width: 4rem;
  height: 4rem;
  margin: 0.5rem;
}

#main-page.is-mobile #side-bar button:first-of-type {
  display: none;
}

#main-page.is-mobile #main-content  {
  margin-left: 0;
  padding-top: 0;
  width: 100vw;
  height: calc(100vh - 5rem);
  overflow-y: auto;
  overflow-x: hidden;
}

#main-page.is-mobile #main-content .content-section {
  padding: 0;
}
</style>
