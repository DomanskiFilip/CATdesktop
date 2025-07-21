<template>
  <h2>notifications:</h2>
  <section class="container">
    <section class="permission-toggle">
      <label class="toggle-switch">
        <input type="checkbox" v-model="useNotifications" @change="handleNotificationsToggle"/>
        <span class="slider"></span>
      </label>
      <span class="toggle-label">Enable Notifications</span>
      <div v-if="errorMessage" class="error">
        {{ errorMessage }}
      </div>
    </section>

    <section class="selector" v-if="useNotifications">
      <label for="select">Select When to Notify:</label>
      <select id="select" v-model="selectedNotificationTimee" @change="handleLeadTimeChange">
        <option :value="null">Choose when to notify...</option>
        <option v-for="type in notificationTime" :key="type" :value="type">
          {{ type }}
        </option>
      </select>
    </section>
    
  </section>
</template>

<script setup lang="ts">
import { ref, watch, onMounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
const useNotifications = ref(true)
const selectedNotificationTimee = ref<string | null>(null)
const notificationTime = ['5 minutes before', '10 minutes before', '15 minutes before', '30 minutes before', '1 hour before']
const timeModes = [5 , 10, 15, 30, 60] // modes coresponding to the notification times to pass to the backend
const errorMessage = ref('')

const handleNotificationsToggle = async () => {
  const lead = selectedNotificationTimee.value
    ? timeModes[notificationTime.indexOf(selectedNotificationTimee.value)]
    : 15;
  await invoke('set_notification_service', { enabled: useNotifications.value, leadMinutes: lead });
}

const handleLeadTimeChange = async () => {
  const lead = selectedNotificationTimee.value
    ? timeModes[notificationTime.indexOf(selectedNotificationTimee.value)]
    : 15;
  await invoke('set_notification_lead_time', { leadMinutes: lead });
};

</script>

<style scoped src="@/assets/checkbox-slider.css"></style>
