<template>
   <form @submit.prevent="register">
    <h2>Register</h2>
    <input v-model="email" type="text" :class="{ 'input-error': emailError }" placeholder="Email" />
    <input v-model="password" type="password" :class="{ 'input-error': passwordError }" placeholder="Password" />
    <input v-model="confirmPassword" type="password" :class="{ 'input-error': confirmPasswordError }" placeholder="Confirm Password" />
    <button type="submit">Register</button>
    <span v-if="loadingOn && !error" id="loader"></span>
    <span v-if="loadingOn && !error">Logging in..</span>
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
const confirmPassword = ref('')
const confirmPasswordError = ref(false)
const error = ref('')
const loadingOn = ref(false)

// Function to handle registration
function register() {
  error.value = ''
  emailError.value = false
  passwordError.value = false
  confirmPasswordError.value = false
  loadingOn.value = true

  // validation checks
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

  if (password.value.length < 8 || !/\d/.test(password.value) && password.value !== password.value.toLowerCase()) {
      error.value = 'Password must be at least 8 characters long, contain at least one upper letter and a number.'
      passwordError.value = true
      loadingOn.value = false
      return
  }

  if (password.value !== confirmPassword.value) {
    error.value = 'Passwords do not match.'
    passwordError.value = true
    confirmPasswordError.value = true
    loadingOn.value = false
    return
  }

  

  // Call the backend to register the user and handle response
  invoke('register_user', { email: email.value, password: password.value})
  .then((response: any) => {
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
      error.value = 'Registration successful!'
    } else {
      error.value = respObj.message || 'registration failed. Please try again.'
      email.value = ''
      password.value = ''
      confirmPassword.value = ''
      loadingOn.value = false
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
    loadingOn.value = false
  })
}
</script>

<style scoped src="@/assets/loginform.css"></style>
