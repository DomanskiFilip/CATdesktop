<template>
   <form @submit.prevent="login">
    <h2>Login</h2>
    <input v-model="email" type="text" :class="{ 'input-error': emailError }" placeholder="Email" />
    <input v-model="password" type="password" :class="{ 'input-error': passwordError }" placeholder="Password" />
    <button type="submit">Login</button>
    <span v-if="loadingOn && !error" id="loader"></span>
    <span v-if="loadingOn && !error">Logging in..</span>
    <p v-if="error" class="error">{{ error }}</p>
  </form>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { platform } from '@tauri-apps/plugin-os'
import { checkStatus, BiometryType, type Status } from "@tauri-apps/plugin-biometric";
import { store as keystoreStore } from '@impierce/tauri-plugin-keystore'

const email = ref('')
const emailError = ref(false)
const password = ref('')
const passwordError = ref(false)
const error = ref('')
const loadingOn = ref(false)

const emit = defineEmits(['updateLoggedIn']);

async function login() {
  error.value = ''
  emailError.value = false
  passwordError.value = false
  loadingOn.value = true

  if (!/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email.value)) {
    error.value = 'Please enter a valid email address.'
    emailError.value = true
    loadingOn.value = false
    return
  }

  if (!password.value) {
    error.value = 'Please enter password.'
    passwordError.value = true
    loadingOn.value = false
    return
  }

  try {
    const os = await platform();
    if (os === 'android' || os === 'ios') {
      // Check biometric status BEFORE login or keystore usage
      const biometricsStatus: Status = await checkStatus();
      if (biometricsStatus.biometryType === BiometryType.None) {
        error.value = "Please set up a biometric (fingerprint, face, etc.) on your device to enable secure login.";
        loadingOn.value = false;
        return;
      }
    }

    const response: any = await invoke('login_user', { email: email.value, password: password.value })
    let respObj = response
    if (typeof response === 'string') {
      try {
        respObj = JSON.parse(response)
      } catch (e) {
        error.value = 'Unexpected response from server.'
        loadingOn.value = false
        return
      }
    }

    if (respObj.status === 'ok') {
      if (os === 'android' || os === 'ios') {
        const tokensToStore = {
          access_token: respObj.tokens?.access_token || null,
          refresh_token: respObj.tokens?.refresh_token || null,
          user_id: respObj.user_id || null,
          database_token: respObj.database_token || null,
        };
        await keystoreStore(JSON.stringify(tokensToStore));
        // Cache user_id in localStorage for quick access
        if (respObj.user_id) {
          localStorage.setItem('cachedUserId', respObj.user_id);
          await invoke('set_user_id_for_backend', { userId: respObj.user_id });
        }
      }
      error.value = 'Log in successful!';
      emit('updateLoggedIn', true);
    }
    loadingOn.value = false
  } catch (err) {
    console.error("Login error (raw):", err)
    let msg = 'An error during login occurred'
    // Show a user-friendly message for biometric errors
    if (typeof err === 'string' && err.includes('biometric')) {
      msg = "Please set up a biometric (fingerprint, face, etc.) on your device to enable secure login.";
    } else if (typeof err === 'string') {
      try {
        const parsed = JSON.parse(err)
        msg = parsed.message || msg
      } catch (_) {
        msg = err
      }
    }
    error.value = msg
    loadingOn.value = false
  }
}
</script>

<style scoped src="@/assets/loginform.css"></style>
