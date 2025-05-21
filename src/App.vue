<template>
  <div id="theme-background-element"></div>
  <section id="side-bar"></section> <!-- future component -->
  <section id="sync-page"> <!-- future component -->
    <google-oauth />
  </section> 
  
</template>

<script setup lang="ts">
import GoogleOauth from './components/googleOauth.vue'

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
</style>
