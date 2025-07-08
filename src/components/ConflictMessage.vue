<template>
  <div class="conflict-dialog" v-if="isVisible && newSuggestion">
    <div class="conflict-overlay" @click="$emit('cancel')"></div>
    <div class="conflict-content">
      <h3>Event Conflict Detected</h3>
      <div class="conflict-details">
        <p v-if="conflictType === 'time'">
          There's already an event at this time:
        </p>
        <p v-else-if="conflictType === 'description'">
          Found existing event with matching description:
        </p>
        
        <div class="event-comparison">
          <div class="existing-event">
            <h4>Existing Event:</h4>
            <p><strong>Description:</strong> {{ existingEvent?.description }}</p>
            <p><strong>Time:</strong> {{ formatDate(existingEvent?.time) }}</p>
          </div>
          
          <div class="new-event">
            <h4>New Event:</h4>
            <p><strong>Description:</strong> {{ newSuggestion.description }}</p>
            <p v-if="newSuggestion.time">
              <strong>Time:</strong> {{ formatDate(newSuggestion.time) }}
            </p>
            <p><strong>Alarm:</strong> {{ newSuggestion.alarm ? 'Yes' : 'No' }}</p>
          </div>
        </div>
      </div>
      
      <div class="conflict-actions">
        <button @click="$emit('update')" class="update-btn">
          Update Existing Event
        </button>
        <button @click="$emit('cancel')" class="cancel-btn">
          Keep Original
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { format } from 'date-fns'

interface EventSuggestion {
  description: string;
  time?: string;
  alarm: boolean;
  recurrence?: string;
}

interface Props {
  isVisible: boolean;
  existingEvent: any;
  newSuggestion: EventSuggestion | null;
  conflictType: string;
}

defineProps<Props>();
defineEmits<{
  update: [];
  cancel: [];
}>();

// Format date for display
const formatDate = (dateString: string | undefined) => {
  if (!dateString) return 'No time specified';
  try {
    const date = new Date(dateString);
    return format(date, 'EEEE, MMMM d, yyyy \'at\' h:mm a');
  } catch (e) {
    return dateString;
  }
};
</script>

<style scoped>
.conflict-dialog {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  z-index: 1000;
  display: flex;
  align-items: center;
  justify-content: center;
}

.conflict-overlay {
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background-color: rgba(0, 0, 0, 0.5);
}

.conflict-content {
  position: relative;
  background-color: var(--color-main);
  border-radius: 8px;
  padding: 1.5rem;
  max-width: 500px;
  width: 90%;
  box-shadow: 0 4px 20px rgba(0, 0, 0, 0.3);
}

.conflict-content h3 {
  margin-top: 0;
  margin-bottom: 1rem;
  color: var(--color-theme);
  text-align: center;
}

.conflict-details {
  margin-bottom: 1.5rem;
}

.event-comparison {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 1rem;
  margin-top: 1rem;
}

.existing-event,
.new-event {
  background-color: rgba(0, 0, 0, 0.05);
  border-radius: 6px;
  padding: 0.75rem;
}

.existing-event h4,
.new-event h4 {
  margin-top: 0;
  margin-bottom: 0.5rem;
  font-size: 0.9rem;
}

.existing-event h4 {
  color: #ff6b6b;
}

.new-event h4 {
  color: #51cf66;
}

.existing-event p,
.new-event p {
  margin: 0.25rem 0;
  font-size: 0.85rem;
}

.conflict-actions {
  display: flex;
  gap: 0.75rem;
  justify-content: center;
}

.update-btn {
  background-color: var(--color-theme);
  color: var(--color-dark);
  border: none;
  padding: 0.75rem 1.5rem;
  border-radius: 6px;
  cursor: pointer;
  font-weight: bold;
}

.update-btn:hover {
  opacity: 0.9;
}

.cancel-btn {
  background-color: rgba(0, 0, 0, 0.1);
  color: var(--color-text);
  border: 1px solid rgba(0, 0, 0, 0.2);
  padding: 0.75rem 1.5rem;
  border-radius: 6px;
  cursor: pointer;
}

.cancel-btn:hover {
  background-color: rgba(0, 0, 0, 0.15);
}
</style>
