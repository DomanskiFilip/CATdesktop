<template>
  <div class="event-suggestion">
    <div v-if="isUpdate || isMoved" class="update-comparison">
      <h4>{{ isMoved ? 'Event Move' : 'Event Update' }}</h4>
      <div class="comparison-grid">
        <div class="old-event">
          <h5>Current Event:</h5>
          <p><strong>Description:</strong> {{ currentEvent?.description || 'Loading...' }}</p>
          <p v-if="currentEvent?.time"><strong>Time:</strong> {{ formatTime(currentEvent.time) }}</p>
          <p><strong>Alarm:</strong> {{ currentEvent?.alarm ? 'Yes' : 'No' }}</p>
          <p v-if="currentEvent?.recurrence"><strong>Recurrence:</strong> {{ currentEvent.recurrence }}</p>
        </div>
        <div class="arrow">→</div>
        <div class="new-event">
          <h5>{{ isMoved ? 'Moved Event:' : 'Updated Event:' }}</h5>
          <p><strong>Description:</strong> {{ eventSuggestion.description }}</p>
          <p v-if="eventSuggestion.time" :class="{'highlight-change': isMoved}"><strong>Time:</strong> {{ formatTime(eventSuggestion.time) }}</p>
          <p><strong>Alarm:</strong> {{ eventSuggestion.alarm ? 'Yes' : 'No' }}</p>
          <p v-if="eventSuggestion.recurrence"><strong>Recurrence:</strong> {{ eventSuggestion.recurrence }}</p>
        </div>
      </div>
    </div>
    <div v-else class="new-event-suggestion">
      <h4>New Event Suggestion</h4>
      <div class="event-details">
        <p><strong>Description:</strong> {{ eventSuggestion.description }}</p>
        <p v-if="eventSuggestion.time"><strong>Time:</strong> {{ formatTime(eventSuggestion.time) }}</p>
        <p><strong>Alarm:</strong> {{ eventSuggestion.alarm ? 'Yes' : 'No' }}</p>
        <p v-if="eventSuggestion.recurrence"><strong>Recurrence:</strong> {{ eventSuggestion.recurrence }}</p>
      </div>
    </div>
    
    <div v-if="eventAccepted" class="suggestion-status accepted">
      ✓ {{ getAcceptedStatusText() }}
    </div>
    <div v-else-if="eventRejected" class="suggestion-status rejected">
      ✗ {{ getRejectedStatusText() }}
    </div>
    <div v-else class="suggestion-actions">
      <button @click="$emit('accept')" class="accept-btn">
        {{ getAcceptButtonText() }}
      </button>
      <button @click="$emit('reject')" class="reject-btn">
        {{ getRejectionButtonText() }}
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'

interface EventSuggestion {
  target_event_id?: string;
  description: string;
  time?: string;
  alarm: boolean;
  recurrence?: string;
}

interface Props {
  eventSuggestion: EventSuggestion;
  eventAccepted?: boolean;
  eventRejected?: boolean;
  isUpdate?: boolean;
  isMoved?: boolean;
  isDelete?: boolean;
}

const props = defineProps<Props>();
const currentEvent = ref<any>(null);

const formatTime = (timeString: string) => {
  return new Date(timeString).toLocaleString();
};

// Helper functions for button and status text
const getAcceptButtonText = () => {
  if (props.isMoved) return 'Move Event';
  if (props.isUpdate) return 'Update Event';
  return 'Add to Calendar';
};

const getRejectionButtonText = () => {
  if (props.isMoved) return 'Keep Current Time';
  if (props.isUpdate) return 'Keep Current';
  return 'Reject';
};

const getAcceptedStatusText = () => {
  if (props.isMoved) return 'Event Moved';
  if (props.isUpdate) return 'Event Updated';
  return 'Event Added to Calendar';
};

const getRejectedStatusText = () => {
  if (props.isMoved) return 'Move Cancelled';
  if (props.isUpdate) return 'Update Cancelled';
  return 'Event Rejected';
};

// Fetch current event details by id only
const loadCurrentEvent = async () => {
  if (!props.isUpdate && !props.isMoved) return;
  try {
    if (props.eventSuggestion.target_event_id) {
      const eventsJson = await invoke<string[]>('get_events');
      const events = eventsJson.map(eventStr => JSON.parse(eventStr));
      const match = events.find(e => e.id === props.eventSuggestion.target_event_id);
      if (match) {
        currentEvent.value = match;
      }
    }
  } catch (error) {
    console.error('Error loading current event:', error);
  }
};

onMounted(() => {
  loadCurrentEvent();
});
</script>

<style scoped>
.event-suggestion {
  background-color: rgba(0, 0, 0, 0.05);
  border-radius: 6px;
  padding: 0.75rem;
  margin-bottom: 0.75rem;
}

.update-comparison {
  margin-bottom: 1rem;
}

.comparison-grid {
  display: grid;
  grid-template-columns: 1fr auto 1fr;
  gap: 1rem;
  align-items: center;
}

.old-event, .new-event {
  padding: 0.5rem;
  border-radius: 4px;
}

.old-event {
  background-color: rgba(255, 0, 0, 0.1);
  border-left: 3px solid #ff6b6b;
}

.new-event {
  background-color: rgba(0, 255, 0, 0.1);
  border-left: 3px solid #51cf66;
}

.arrow {
  font-size: 1.5rem;
  font-weight: bold;
  color: var(--color-theme);
}

.highlight-change {
  font-weight: bold;
  color: #1a73e8;
  background-color: rgba(26, 115, 232, 0.1);
  padding: 2px 4px;
  border-radius: 3px;
}

.new-event-suggestion {
  margin-bottom: 1rem;
}

.event-details p {
  margin: 0.25rem 0;
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

.accept-btn {
  background-color: rgba(0, 255, 0, 0.2);
  border: none;
  cursor: pointer;
  color: green;
  padding: 0.5rem 1rem;
  border-radius: 4px;
}

.reject-btn {
  background-color: rgba(255, 0, 0, 0.2);
  border: none;
  cursor: pointer;
  color: darkred;
  padding: 0.5rem 1rem;
  border-radius: 4px;
}

@media (max-width: 768px) {
  .comparison-grid {
    grid-template-columns: 1fr;
    gap: 0.5rem;
  }
  
  .arrow {
    text-align: center;
    transform: rotate(90deg);
  }
}
</style>