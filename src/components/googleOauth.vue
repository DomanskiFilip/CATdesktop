<template>
  <section>
    <button v-if="!loggedIn" @click="checkOAuthAndLoad" :disabled="authenticating">
      login with <img src="@/assets/google-2025-g-logo.webp" alt="Google Logo" class="google-logo"/>
    </button>
    <div v-if="authenticating && timer > 0">
      Authenticating with google ...
      Time left: {{ timer }} seconds
    </div>
    <div id="endpoint" v-html="message"></div>
  </section>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'

const message = ref('')
const timer = ref(0)
const authenticating = ref(false)
let interval: number | undefined
const oauthFinished = ref(false)
const loggedIn = ref(false)

async function checkOAuthAndLoad() {
  message.value = ''
  authenticating.value = true
  oauthFinished.value = false

  // Fetch timeout from backend
  timer.value = await invoke<number>('get_oauth_timeout')

  // Start countdown timer
  interval = window.setInterval(() => {
    if (timer.value > 0) {
      timer.value--
    } else {
      clearInterval(interval)
      if (!oauthFinished.value) {
        authenticating.value = false
        message.value = 'OAuth failed or cancelled. Please try again.'
      }
    }
  }, 1000)

  // Start OAuth process in parallel with timer
  try {
    const oauthResult = await invoke<string>('run_oauth2_flow')
    if (!oauthFinished.value) {
      oauthFinished.value = true
      authenticating.value = false
      clearInterval(interval)
      if (oauthResult) {
        message.value = '<span>OAuth successful!</span>'
        const body = await invoke<string>('fetch_lambda_endpoint')
        if (!body) throw new Error('No data returned')
        const outer = JSON.parse(body)
        const response = typeof outer.body === 'string' ? JSON.parse(outer.body) : outer.body
        if (response.status === 'ok') {
          // Replace the message, removing "Loading data..."
          let msg = `<span>OAuth successful!</span><span>Lambda connection: ${response.message}</span>`
          if (Array.isArray(response.items)) {
            response.items.forEach((item: any) => {
              msg += `<span>id: ${item.id}, text: ${item.text}</span>`
            })
          } else {
            msg += 'No items found.'
          }
          message.value = msg
          loggedIn.value = true
        } else {
          message.value = `<span>OAuth successful!</span>Lambda error: ${response.message || 'Unknown error'}`
        }
      }
    }
  } catch (err) {
    if (!oauthFinished.value) {
      oauthFinished.value = true
      authenticating.value = false
      clearInterval(interval)
      message.value = 'OAuth failed or cancelled. Please try again.'
    }
  }
}
</script>

<style scoped>
section {
  display: flex;
  align-items: center;
  justify-content: flex-start;
  gap: 1rem;
}
button {
  margin: 1rem;
  margin-left: 0.5rem;
  padding: 0.5rem 1.5rem;
  font-size: 1rem;
  cursor: pointer;
  display: flex;
  align-items: center;
}
#endpoint {
  display: flex;
  flex-direction: column;
  justify-content: center;
  align-items: center;
  margin-top: 1rem;
}
.google-logo {
  width: 24px;
  height: 24px;
  vertical-align: middle;
}
</style>