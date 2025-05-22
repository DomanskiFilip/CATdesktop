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

const email = ref('')
const emailError = ref(false)
const password = ref('')
const passwordError = ref(false)
const error = ref('')

function login() {
  error.value = ''
  if (!email.value || !password.value) {
    error.value = 'Please fill in all fields.'
    emailError.value = true
    passwordError.value = true
    return
  }

  invoke('login', { email: email.value, password: password.value })
    .then((response: any) => {
      if (response.status === 'ok') {
        error.value = 'login successful!'
      } else {
        error.value = response.message || 'Login failed. Please try again.'
        email.value = ''
        password.value = ''
      }
    })
    .catch((err) => {
      error.value = 'An error occurred'
      console.error('Error during login:', err)
    })
}
</script>

<style scoped src="@/assets/loginform.css"></style>