<template>
  <div id="theme-background-element"></div> <!-- background visual element -->
  <section v-if="loggedIn" id="main-page">
    <section id="side-bar"></section> <!-- future component -->
    <section id="sync-page"> <!-- future component -->
      <google-oauth />
    </section> 
  </section>
  
  <!-- login/register page -->
  <section v-else id="login-register-page">
    <login v-if="showLogin" @updateLoggedIn="loggedIn = $event"/>
    <register v-else />
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
  
  
</template>

<script setup lang="ts">
import { ref } from 'vue'
import GoogleOauth from './components/googleOauth.vue'
import login from './components/Login.vue'
import register from './components/Register.vue'

const showLogin = ref(true)
const loggedIn = ref(false)
</script>

<style scoped>
  #sync-page {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 100vh;
    gap: 1rem;
  }

  #login-register-page {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 100vh;
    width: 100vw;
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
</style>
