<template>
   <form @submit.prevent="register">
    <h2>Register</h2>
    <input v-model="email" type="text" :class="{ 'input-error': emailError }" placeholder="Email" />
    <input v-model="password" type="password" :class="{ 'input-error': passwordError }" placeholder="Password" />
    <input v-model="confirmPassword" type="password" :class="{ 'input-error': confirmPasswordError }" placeholder="Confirm Password" />
    <button type="submit">Register</button>
    <p v-if="error" class="error">{{ error }}</p>
  </form>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'

const email = ref('')
const emailError = ref(false)
const password = ref('')
const passwordError = ref(false)
const confirmPassword = ref('')
const confirmPasswordError = ref(false)
const error = ref('')

function register() {
  error.value = ''
  emailError.value = false
  passwordError.value = false
  confirmPasswordError.value = false

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

  if (password.value.length < 8 || !/\d/.test(password.value) && password.value !== password.value.toLowerCase()) {
      error.value = 'Password must be at least 8 characters long, contain at least one upper letter and a number.'
      passwordError.value = true
      return
  }

  if (password.value !== confirmPassword.value) {
    error.value = 'Passwords do not match.'
    passwordError.value = true
    confirmPasswordError.value = true
    return
  }

  


  invoke('register_user', { email: email.value, password: password.value})
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
      error.value = 'Registration successful!'
    } else {
      error.value = respObj.message || 'registration failed. Please try again.'
      email.value = ''
      password.value = ''
      confirmPassword.value = ''
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
    console.error('Error during registration:', err)
  })
}
</script>

<style scoped src="@/assets/loginform.css"></style>