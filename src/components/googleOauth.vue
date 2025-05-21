<template>
  <section>
    <button @click="checkOAuthAndLoad" :disabled="authenticating">login with <img src="@/assets/google-2025-g-logo.webp" alt="Google Logo" class="google-logo"/></button>
    <div v-if="authenticating && timer > 0">
      Authenticating with google ...
      Time left: {{ timer }} seconds
    </div>
    <div id="endpoint" v-html="message"></div>
  </section>
</template>

<script lang="ts">
import { defineComponent } from 'vue'
import { invoke } from '@tauri-apps/api/core'

export default defineComponent({
  data() {
    return {
      message: '',
      timer: 0,
      authenticating: false,
      interval: undefined as number | undefined,
      oauthFinished: false,
    }
  },
  computed: {
    timeLeft(): number {
      return this.timer
    }
  },
  methods: {
    async checkOAuthAndLoad() {
      this.message = ''
      this.authenticating = true
      this.oauthFinished = false

      // Fetch timeout from backend
      const timeout = await invoke<number>('get_oauth_timeout')
      this.timer = timeout

      // Start countdown timer
      this.interval = window.setInterval(() => {
        if (this.timer > 0) {
          this.timer--
        } else {
          clearInterval(this.interval)
          if (!this.oauthFinished) {
            this.authenticating = false
            this.message = 'OAuth failed or cancelled. Please try again.'
          }
        }
      }, 1000)

      // Start OAuth process in parallel with timer
      try {
        const oauthResult = await invoke<string>('run_oauth2_flow')
        if (!this.oauthFinished) {
          this.oauthFinished = true
          this.authenticating = false
          clearInterval(this.interval)
          if (oauthResult) {
                this.message = '<span>OAuth successful!</span><span>Loading data...</span>'
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
                    this.message = msg
                } else {
                    this.message = `<span>OAuth successful!</span>Lambda error: ${response.message || 'Unknown error'}`
                }
            }
        }
      } catch (err) {
        if (!this.oauthFinished) {
          this.oauthFinished = true
          this.authenticating = false
          clearInterval(this.interval)
          this.message = 'OAuth failed or cancelled. Please try again.'
        }
      }
    }
  }
})
</script>

<style scoped>
section {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
}
button {
  margin-top: 1rem;
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
}
.google-logo {
  width: 24px;
  height: 24px;
  vertical-align: middle;
}
</style>