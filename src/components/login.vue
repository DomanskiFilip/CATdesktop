<template>
   <form @submit.prevent="login">
    <h2>Login</h2>
    <input v-model="email" type="text" :class="{ 'input-error': emailError }" placeholder="Email" />
    <input v-model="password" type="password" :class="{ 'input-error': passwordError }" placeholder="Password" />
    <button type="submit">Login</button>
    <p v-if="error" class="error">{{ error }}</p>
  </form>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'

// error values
const email = ref('')
const emailError = ref(false)
const password = ref('')
const passwordError = ref(false)
const error = ref('')

// Function to handle login
function login() {
  error.value = ''
  emailError.value = false
  passwordError.value = false

  // validation checks
  if (!/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email.value)) {
    error.value = 'Please enter a valid email address.'
    emailError.value = true
    return
  }

  if (!password.value) {
    error.value = 'Please enter password.'
    passwordError.value = true
    return
  }

  // Call the backend to login the user and handle response
  invoke('login_user', { email: email.value, password: password.value })
    .then((response: any) => {
    let respObj = response
    if (typeof response === 'string') {
      try {
        respObj = JSON.parse(response)
      } catch (e) {
        error.value = 'Unexpected response from server.'
        return
      }
    }
    if (respObj.status === 'ok') {
      error.value = 'Login successful!'
    } else {
      error.value = respObj.message || 'Login failed. Please try again.'
      email.value = ''
      password.value = ''
    }
  })
  .catch((err) => {
    let msg = 'An error occurred'
    if (typeof err === 'string') {
      try {
        const parsed = JSON.parse(err)
        msg = parsed.message || msg
      } catch (_) {
        msg = err
      }
    }
    error.value = msg
  })
}
</script>

<style scoped src="@/assets/loginform.css"></style>