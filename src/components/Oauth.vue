<template>
  <section v-if="!isMobile">
    <button v-if="!loggedIn" @click="checkOAuthAndLoad" :disabled="authenticating">
      Login with
      <img  v-if="providerName === 'Google'" src="../assets/google-2025-g-logo.webp" alt="Google Logo" class="oauth-logo"/>
      <img v-else-if="providerName === 'Outlook'"  src="../assets/outlook-logo.webp" alt="Outlook Logo" class="oauth-logo"/>
    </button>
    <div v-if="authenticating && timer > 0">
      Authenticating with {{ providerName }}...
      Time left: {{ timer }} seconds
    </div>
    <div id="endpoint" v-html="message"></div>
  </section>
</template>

<script setup lang="ts">
import { ref, defineProps, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'

// Props for customization
const props = defineProps({
  providerName: {
    type: String,
    required: true,
    default: 'Google', // Default to Google
  },
})

const isMobile = computed(() => /Android|webOS|iPhone|iPad|iPod|BlackBerry|IEMobile|Opera Mini/i.test(navigator.userAgent))
const providerName = props.providerName
const message = ref('')
const timer = ref(0) // Timer will be set dynamically
const authenticating = ref(false)
let interval: number | undefined
const oauthFinished = ref(false)
const loggedIn = ref(false)

async function checkOAuthAndLoad() {
  message.value = ''
  authenticating.value = true
  oauthFinished.value = false

  try {
    // Fetch the timeout dynamically from the backend
    timer.value = await invoke<number>('get_oauth_timeout')

    // Start countdown timer
    interval = window.setInterval(() => {
      if (timer.value > 0) {
        timer.value--
      } else {
        clearInterval(interval)
        if (!oauthFinished.value) {
          authenticating.value = false
          message.value = `${providerName} OAuth failed or cancelled. Please try again.`
        }
      }
    }, 1000)

    // Determine the OAuth flow function based on the provider
    const oauthFlowFunction =
      providerName === 'Google' ? 'run_oauth2_flow' : 'run_outlook_oauth2_flow'

    // Start OAuth process in parallel with timer
    const oauthResult = await invoke<string>(oauthFlowFunction)
    if (!oauthFinished.value) {
      oauthFinished.value = true
      authenticating.value = false
      clearInterval(interval)
      if (oauthResult) {
        message.value = `<span>${providerName} OAuth successful!</span>`
        loggedIn.value = true
      }
    }
  } catch (err) {
    if (!oauthFinished.value) {
      oauthFinished.value = true
      authenticating.value = false
      clearInterval(interval)
      message.value = `${providerName} OAuth failed or cancelled. Please try again.`
    }
  }
}
</script>

<style scoped>
h2 {
  color: var(--color-text);
  font-size: 1.2rem;
  margin-left: 0.5rem;
}

section {
  display: flex;
  align-items: center;
  justify-content: flex-start;
  gap: 1rem;
  margin-left: 0.5rem;
}
button {
  margin: 1rem;
  margin-left: 0.5rem;
  padding: 0.5rem 1.5rem;
  font-size: 1rem;
  cursor: pointer;
  display: flex;
  align-items: center;
  background-color: var(--color-shadow);
  border: none;
}

button:hover {
  box-shadow: 0 0 10px var(--color-theme);
}

#endpoint {
  display: flex;
  flex-direction: column;
  justify-content: center;
  align-items: center;
  margin-top: 1rem;
}
.oauth-logo {
  width: 24px;
  height: 24px;
  vertical-align: middle;
}
</style>