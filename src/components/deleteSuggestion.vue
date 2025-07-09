<template>
  <div class="delete-suggestion">
    <div class="delete-confirmation">
      <h4>🗑️ Delete Event</h4>
      <div class="event-to-delete">
        <p v-if="currentEvent">
          <strong>Are you sure you want to delete:</strong>
        </p>
        <div v-if="currentEvent" class="event-details">
          <p><strong>Description:</strong> {{ currentEvent.description }}</p>
          <p v-if="currentEvent.time"><strong>Time:</strong> {{ formatTime(currentEvent.time) }}</p>
          <p><strong>Alarm:</strong> {{ currentEvent.alarm ? 'Yes' : 'No' }}</p>
          <p v-if="currentEvent.recurrence"><strong>Recurrence:</strong> {{ currentEvent.recurrence }}</p>
        </div>
        <div v-else class="loading">
          <p>Looking for event: <strong>{{ eventSuggestion.description }}</strong></p>
        </div>
      </div>
    </div>
    
    <div v-if="eventAccepted" class="suggestion-status accepted">
      ✓ Event Deleted
    </div>
    <div v-else-if="eventRejected" class="suggestion-status rejected">
      ✗ Deletion Cancelled
    </div>
    <div v-else class="suggestion-actions">
      <button @click="$emit('accept')" class="delete-btn" :disabled="!currentEvent">
        Delete Event
      </button>
      <button @click="$emit('reject')" class="cancel-btn">
        Cancel
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'

interface EventSuggestion {
  description: string;
  time?: string;
  alarm: boolean;
  recurrence?: string;
}

interface Props {
  eventSuggestion: EventSuggestion;
  eventAccepted?: boolean;
  eventRejected?: boolean;
}

const props = defineProps<Props>();
defineEmits<{
  accept: [];
  reject: [];
}>();

const currentEvent = ref<any>(null);

const formatTime = (timeString: string) => {
  return new Date(timeString).toLocaleString();
};

// Find the event to be deleted
const loadEventToDelete = async () => {
  try {
    const eventsJson = await invoke<string[]>('get_events_for_ai');
    const events = eventsJson.map(eventStr => JSON.parse(eventStr));
    
    // Find the matching event using description
    const description = props.eventSuggestion.description.toLowerCase();
    const match = events.find(e => 
      e.description.toLowerCase() === description ||
      e.description.toLowerCase().includes(description) ||
      description.includes(e.description.toLowerCase())
    );
    
    if (match) {
      currentEvent.value = match;
    }
  } catch (error) {
    console.error('Error loading event to delete:', error);
  }
};

onMounted(() => {
  loadEventToDelete();
});
</script>

<style scoped>
.delete-suggestion {
  background-color: rgba(255, 0, 0, 0.05);
  border: 1px solid rgba(255, 0, 0, 0.2);
  border-radius: 6px;
  padding: 0.75rem;
  margin-bottom: 0.75rem;
}

.delete-confirmation {
  margin-bottom: 1rem;
}

.delete-confirmation h4 {
  margin-top: 0;
  margin-bottom: 0.5rem;
  color: #dc3545;
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

.event-to-delete {
  background-color: rgba(255, 0, 0, 0.08);
  border-radius: 4px;
  padding: 0.75rem;
  border-left: 3px solid #dc3545;
}

.event-details p {
  margin: 0.25rem 0;
}

.loading {
  font-style: italic;
  color: #666;
}

.suggestion-actions {
  display: flex;
  gap: 0.5rem;
}

.suggestion-status {
  padding: 0.5rem;
  text-align: center;
  border-radius: 4px;
  font-weight: bold;
}

.suggestion-status.accepted {
  background-color: rgba(0, 255, 0, 0.2);
  color: green;
}

.suggestion-status.rejected {
  background-color: rgba(255, 0, 0, 0.2);
  color: darkred;
}

.delete-btn {
  background-color: #dc3545;
  color: white;
  border: none;
  cursor: pointer;
  padding: 0.5rem 1rem;
  border-radius: 4px;
  font-weight: bold;
}

.delete-btn:hover:not(:disabled) {
  background-color: #c82333;
}

.delete-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.cancel-btn {
  background-color: rgba(0, 0, 0, 0.1);
  color: var(--color-text);
  border: 1px solid rgba(0, 0, 0, 0.2);
  cursor: pointer;
  padding: 0.5rem 1rem;
  border-radius: 4px;
}

.cancel-btn:hover {
  background-color: rgba(0, 0, 0, 0.15);
}
</style>

