<template>
  <section class="ai-assistant">
    <!-- Chat clear button -->
    <div class="chat-header">
      <h2>Calendar Assistant</h2>
      <button @click="clearChat" class="clear-btn" title="Clear conversation">
        <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="16" height="16">
          <path fill="none" d="M0 0h24v24H0z"/>
          <path d="M17 6h5v2h-2v13a1 1 0 01-1 1H5a1 1 0 01-1-1V8H2V6h5V3a1 1 0 011-1h8a1 1 0 011 1v3zm1 2H6v12h12V8zm-9 3h2v6H9v-6zm4 0h2v6h-2v-6zM9 4v2h6V4H9z" fill="currentColor"/>
        </svg>
        Clear Chat
      </button>
    </div>

    <section class="chat-container" ref="chatContainer">
      <div v-for="(message, index) in chatHistory" :key="index" :class="['message', message.sender === 'user' ? 'user-message' : 'assistant-message']">
        <EventSuggestion
          v-if="message.sender === 'assistant' && message.eventSuggestion"
          :eventSuggestion="message.eventSuggestion"
          :eventAccepted="message.eventAccepted"
          :eventRejected="message.eventRejected"
          :isUpdate="message.isUpdate"
          :isMoved="message.isMoved"
          @accept="acceptSuggestion(index)"
          @reject="rejectSuggestion(index)"
        />
        <div class="message-content">{{ message.content }}</div>
      </div>
      <div v-if="isTyping" class="typing-indicator">
        <span></span>
        <span></span>
        <span></span>
      </div>
    </section>
    
    <section class="input-area">
      <input 
        type="text" 
        v-model="userInput" 
        @keyup.enter="sendMessage"
        placeholder="Ask me to create events, check your schedule, etc."
        :disabled="isProcessing"
      />
      <button @click="sendMessage" :disabled="isProcessing || !userInput.trim()">
        <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="24" height="24">
          <path fill="none" d="M0 0h24v24H0z"/>
          <path d="M3 13h6v-2H3V1.846a.5.5 0 0 1 .741-.438l18.462 10.154a.5.5 0 0 1 0 .876L3.741 22.592A.5.5 0 0 1 3 22.154V13z" fill="var(--color-dark)"/>
        </svg>
      </button>
    </section>

    <!-- Conflict Resolution Dialog -->
    <ConflictMessage
      :isVisible="showConflictDialog"
      :existingEvent="conflictData.existingEvent"
      :newSuggestion="conflictData.newSuggestion"
      :conflictType="conflictData.conflictType"
      @update="handleConflictUpdate"
      @cancel="handleConflictCancel"
    />
  </section>
</template>

<script setup lang="ts">
import { ref, onMounted, nextTick, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { emit as tauriEmit } from '@tauri-apps/api/event'
import EventSuggestion from './EventSuggestion.vue'
import ConflictMessage from './ConflictMessage.vue'

interface EventSuggestion {
  description: string;
  time?: string;
  alarm: boolean;
  recurrence?: string;
}

interface ChatMessage {
  content: string;
  sender: 'user' | 'assistant';
  eventSuggestion?: EventSuggestion;
  eventAccepted?: boolean;
  eventRejected?: boolean;
  isUpdate?: boolean;
  isMoved?: boolean;
}

const chatHistory = ref<ChatMessage[]>([
  {
    content: "Hello! I'm your Calendar AssistanT, You can call me CAT. How can I help you manage your schedule today?",
    sender: 'assistant'
  }
]);
const userInput = ref('');
const isProcessing = ref(false);
const isTyping = ref(false);
const chatContainer = ref<HTMLElement | null>(null);

// Conflict dialog state
const showConflictDialog = ref(false);
const conflictData = ref<{
  existingEvent: any;
  newSuggestion: EventSuggestion | null;
  conflictType: string;
  messageIndex: number;
}>({
  existingEvent: null,
  newSuggestion: null,
  conflictType: '',
  messageIndex: -1
});

const clearChat = () => {
  // Reset chat history to just the welcome message
  chatHistory.value = [{
    content: "Hello! I'm your Calendar AssistanT, You can call me CAT. How can I help you manage your schedule today?",
    sender: 'assistant'
  }];
};

// Scroll to bottom of chat //
const scrollToBottom = async () => {
  await nextTick();
  if (chatContainer.value) {
    chatContainer.value.scrollTop = chatContainer.value.scrollHeight;
  }
};

// Send message to AI //
const sendMessage = async () => {
  if (!userInput.value.trim() || isProcessing.value) return;
  
  const message = userInput.value.trim();
  userInput.value = '';
  
  chatHistory.value.push({
    content: message,
    sender: 'user'
  });
  
  isProcessing.value = true;
  isTyping.value = true;
  
  await scrollToBottom();
  
  try {
    const response = await invoke<string>('process_ai_message', { 
      query: message,
      conversationHistory: JSON.stringify(chatHistory.value.slice(0, -1))
    });
    
    isTyping.value = false;
    
    const aiResponse = JSON.parse(response);
    
    // If there are extracted events, check for conflicts immediately
    if (aiResponse.extracted_events && aiResponse.extracted_events.length > 0) {
      const eventSuggestion = aiResponse.extracted_events[0];
      const isUpdate = aiResponse.action_taken === 'update_event';
      const isMove = aiResponse.action_taken === 'move_event';
      
      // Check for conflicts before showing the suggestion
      const conflictResult = await handleEventConflict(eventSuggestion);
      
      if (conflictResult.hasConflict && conflictResult.conflictingEvent && !isUpdate && !isMove) {
        // Show conflict dialog directly instead of event suggestion
        conflictData.value = {
          existingEvent: conflictResult.conflictingEvent,
          newSuggestion: eventSuggestion,
          conflictType: conflictResult.conflictType || 'unknown',
          messageIndex: chatHistory.value.length
        };
        
        chatHistory.value.push({
          content: aiResponse.response_text,
          sender: 'assistant'
        });
        
        showConflictDialog.value = true;
      } else {
        // No conflict or it's an update/move, show normal event suggestion
        chatHistory.value.push({
          content: aiResponse.response_text,
          sender: 'assistant',
          eventSuggestion: eventSuggestion,
          isUpdate: isUpdate,
          isMoved: isMove
        });
      }
    } else {
      // No events, just show the response
      chatHistory.value.push({
        content: aiResponse.response_text,
        sender: 'assistant'
      });
    }
    
  } catch (error) {
    isTyping.value = false;
    console.error('Error processing message:', error);
    chatHistory.value.push({
      content: "I'm sorry, I encountered an error processing your request. Please try again.",
      sender: 'assistant'
    });
  } finally {
    isProcessing.value = false;
    await scrollToBottom();
  }
};

// Check for conflicts and handle event creation/updates
const handleEventConflict = async (eventSuggestion: EventSuggestion) => {
  try {
    // Get all events to check for conflicts
    const eventsJson = await invoke<string[]>('get_events_for_ai');
    const events = eventsJson.map(eventStr => JSON.parse(eventStr));
    
    const suggestedTime = eventSuggestion.time ? new Date(eventSuggestion.time) : null;
    const suggestedDescription = eventSuggestion.description.toLowerCase().trim();
    
    // Check for conflicts
    let conflictingEvent = null;
    let conflictType = '';
    
    if (suggestedTime) {
      // Check for time conflict (same hour)
      conflictingEvent = events.find(event => {
        const eventTime = new Date(event.time);
        return eventTime.getHours() === suggestedTime.getHours() &&
               eventTime.getDate() === suggestedTime.getDate() &&
               eventTime.getMonth() === suggestedTime.getMonth() &&
               eventTime.getFullYear() === suggestedTime.getFullYear();
      });
      
      if (conflictingEvent) {
        conflictType = 'time';
      }
    }
    
    // Check for description match if no time conflict found
    if (!conflictingEvent) {
      conflictingEvent = events.find(event => 
        event.description.toLowerCase().trim() === suggestedDescription
      );
      
      if (conflictingEvent) {
        conflictType = 'description';
      }
    }
    
    if (conflictingEvent) {
      return { hasConflict: true, conflictingEvent, conflictType };
    }
    
    return { hasConflict: false };
    
  } catch (error) {
    console.error('Error checking for conflicts:', error);
    return { hasConflict: false };
  }
};

// Handle conflict resolution
const handleConflictUpdate = async () => {
  const { existingEvent, newSuggestion, conflictType, messageIndex } = conflictData.value;
  
  if (!newSuggestion) {
    console.error('No suggestion data available for update');
    showConflictDialog.value = false;
    return;
  }
  
  try {
    await updateConflictingEvent(existingEvent, newSuggestion, conflictType);
    
    // Add success message
    chatHistory.value.push({
      content: `Great! I've updated "${newSuggestion.description}" in your calendar.`,
      sender: 'assistant'
    });
    
    // Emit event to refresh calendar
    await tauriEmit('event-saved');
    
  } catch (error) {
    console.error('Error updating conflicting event:', error);
    chatHistory.value.push({
      content: "I'm sorry, I couldn't update this event.",
      sender: 'assistant'
    });
  }
  
  showConflictDialog.value = false;
  await scrollToBottom();
};

const handleConflictCancel = async () => {
  const { messageIndex } = conflictData.value;
  
  const message = chatHistory.value[messageIndex];
  if (message) {
    message.eventRejected = true;
  }
  
  chatHistory.value.push({
    content: "No problem. The existing event remains unchanged. Is there anything else you'd like me to help you with?",
    sender: 'assistant'
  });
  
  showConflictDialog.value = false;
  await scrollToBottom();
};

// Modified acceptSuggestion function
const acceptSuggestion = async (messageIndex: number) => {
  const message = chatHistory.value[messageIndex];
  if (!message || !message.eventSuggestion) return;
  
  try {
    if (message.isUpdate || message.isMoved) {
      await updateExistingEvent(message.eventSuggestion);
    } else {
      await createNewEvent(message.eventSuggestion);
    }
    
    message.eventAccepted = true;
    
    // Emit event to refresh calendar
    await tauriEmit('event-saved');
    
  } catch (error) {
    console.error('Error processing event suggestion:', error);
    chatHistory.value.push({
      content: "I'm sorry, I couldn't process this event suggestion.",
      sender: 'assistant'
    });
  }
  
  await scrollToBottom();
};

// Function to update conflicting event
const updateConflictingEvent = async (existingEvent: any, newSuggestion: EventSuggestion | null, conflictType: string) => {
  if (!newSuggestion) {
    throw new Error('No suggestion data provided');
  }
  
  try {
    let updatedEvent;
    
    if (conflictType === 'time') {
      // Time matches, update description and other properties
      updatedEvent = {
        ...existingEvent,
        description: newSuggestion.description,
        alarm: newSuggestion.alarm !== undefined ? newSuggestion.alarm : existingEvent.alarm,
        recurrence: newSuggestion.recurrence || existingEvent.recurrence,
        synced: false,
        synced_google: false
      };
    } else if (conflictType === 'description') {
      // Description matches, update time and other properties
      updatedEvent = {
        ...existingEvent,
        time: newSuggestion.time || existingEvent.time,
        alarm: newSuggestion.alarm !== undefined ? newSuggestion.alarm : existingEvent.alarm,
        recurrence: newSuggestion.recurrence || existingEvent.recurrence,
        synced: false,
        synced_google: false
      };
    }
    
    // Save the updated event
    await invoke('save_event', { 
      event: JSON.stringify(updatedEvent)
    });
    
    // Handle notifications
    if (updatedEvent.alarm) {
      try {
        await invoke('schedule_event_notification', { 
          event_json: JSON.stringify(updatedEvent) 
        });
      } catch (notificationError) {
        console.warn('Failed to schedule notification:', notificationError);
      }
    }
    
    try {
      await invoke('trigger_sync');
    } catch (syncError) {
      console.warn('Failed to trigger sync:', syncError);
    }
    
  } catch (error) {
    console.error('Error updating conflicting event:', error);
    throw error;
  }
};

// Function to create a new event //
const createNewEvent = async (eventSuggestion: EventSuggestion) => {
  const eventDate = eventSuggestion.time ? 
    new Date(eventSuggestion.time) : 
    new Date(Date.now() + 60 * 60 * 1000);
  
  const calendarEvent = {
    id: crypto.randomUUID(),
    user_id: "",
    description: eventSuggestion.description,
    time: eventDate.toISOString(),
    alarm: eventSuggestion.alarm,
    synced: false,
    synced_google: false,
    deleted: false,
    recurrence: eventSuggestion.recurrence
  };
  
  await invoke('save_event', { 
    event: JSON.stringify(calendarEvent)
  });
  
  if (calendarEvent.alarm) {
    try {
      await invoke('schedule_event_notification', { 
        event_json: JSON.stringify(calendarEvent) 
      });
    } catch (notificationError) {
      console.warn('Failed to schedule notification:', notificationError);
    }
  }
  
  try {
    await invoke('trigger_sync');
  } catch (syncError) {
    console.warn('Failed to trigger sync:', syncError);
  }
  
  chatHistory.value.push({
    content: `Great! I've added "${eventSuggestion.description}" to your calendar.`,
    sender: 'assistant'
  });
};

// Function to update an existing event //
const updateExistingEvent = async (eventSuggestion: EventSuggestion, isMoved: boolean = false) => {
  try {
    // Get all events to find the one to update
    const eventsJson = await invoke<string[]>('get_events_for_ai');
    const events = eventsJson.map(eventStr => JSON.parse(eventStr));
    
    const matchingEvent = findMatchingEvent(events, eventSuggestion);
    
    if (!matchingEvent) {
      throw new Error("Could not find the event to update");
    }
    
    // Handle updating the event differently based on whether it's a move or regular update
    const updatedEvent = {
      ...matchingEvent,
      description: eventSuggestion.description,
      time: eventSuggestion.time || matchingEvent.time,
      alarm: eventSuggestion.alarm !== undefined ? eventSuggestion.alarm : matchingEvent.alarm,
      recurrence: eventSuggestion.recurrence || matchingEvent.recurrence,
      synced: false,
      synced_google: false
    };
    
    // Save the updated event
    await invoke('save_event', { 
      event: JSON.stringify(updatedEvent)
    });
    
    // Handle notifications
    if (updatedEvent.alarm) {
      try {
        await invoke('schedule_event_notification', { 
          event_json: JSON.stringify(updatedEvent) 
        });
      } catch (notificationError) {
        console.warn('Failed to schedule notification:', notificationError);
      }
    }
    
    try {
      await invoke('trigger_sync');
    } catch (syncError) {
      console.warn('Failed to trigger sync:', syncError);
    }
    
    // Customize the success message based on operation type
    let messageText;
    if (isMoved) {
      messageText = `Great! I've moved "${updatedEvent.description}" to ${new Date(updatedEvent.time).toLocaleString()}.`;
    } else {
      // For regular updates, check if time actually changed
      messageText = eventSuggestion.time && eventSuggestion.time !== matchingEvent.time
        ? `Great! I've updated "${updatedEvent.description}" with a new time: ${new Date(updatedEvent.time).toLocaleString()}.`
        : `Great! I've updated "${updatedEvent.description}" in your calendar.`;
    }
    
    chatHistory.value.push({
      content: messageText,
      sender: 'assistant'
    });
    
  } catch (error) {
    console.error('Error updating event:', error);
    throw error;
  }
};

// Helper function to find matching event //
const findMatchingEvent = (events: any[], eventSuggestion: EventSuggestion, originalDescription?: string) => {
  const description = eventSuggestion.description.toLowerCase();
  
  // If we have original description (for updates), search by that first
  if (originalDescription) {
    const match = events.find(e => 
      e.description.toLowerCase() === originalDescription.toLowerCase()
    );
    if (match) return match;
  }
  
  // First try exact description match
  let match = events.find(e => 
    e.description.toLowerCase() === description
  );
  
  if (match) return match;
  
  // Try time match if provided
  if (eventSuggestion.time) {
    const updateTime = new Date(eventSuggestion.time);
    match = events.find(e => {
      const eventTime = new Date(e.time);
      return Math.abs(eventTime.getTime() - updateTime.getTime()) < 30 * 60 * 1000; // 30 minutes
    });
  }
  
  if (match) return match;
  
  // Try partial description match as last resort
  match = events.find(e => {
    const eventDesc = e.description.toLowerCase();
    return eventDesc.includes(description) || description.includes(eventDesc);
  });
  
  return match;
};

// Reject event suggestion //
const rejectSuggestion = async (messageIndex: number) => {
  const message = chatHistory.value[messageIndex];
  if (!message) return;
  
  message.eventRejected = true;
  
  chatHistory.value.push({
    content: "No problem. Is there anything else you'd like me to help you with?",
    sender: 'assistant'
  });
  
  await scrollToBottom();
};

// Scroll to bottom on mount
onMounted(() => {
  scrollToBottom();
    });

// Scroll to bottom when chat history changes
watch(chatHistory, () => {
  scrollToBottom();
});
</script>

<style scoped>
.ai-assistant {
  display: flex;
  flex-direction: column;
  height: 95vh;
  max-width: 800px;
  margin: 0 auto;
  background-color: var(--color-main);
  border-radius: 8px;
}

.chat-container {
  flex: 1;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 1rem;
  margin-bottom: 1rem;
  padding: 1rem;
  background-color: rgba(0, 0, 0, 0.05);
  border-radius: 8px;
}

.message {
  max-width: 80%;
  padding: 0.75rem;
  border-radius: 8px;
  word-break: break-word;
}

.user-message {
  align-self: flex-end;
  background-color: var(--color-theme);
  color: var(--color-dark);
}

.assistant-message {
  align-self: flex-start;
  background-color: rgba(0, 0, 0, 0.1);
}

.input-area {
  display: flex;
  gap: 0.5rem;
  margin-top: auto;
}

.input-area input {
  flex: 1;
  padding: 0.75rem;
  border-radius: 8px;
  border: 1px solid rgba(0, 0, 0, 0.2);
  background-color: rgba(0, 0, 0, 0.05);
  color: var(--color-text);
}

.input-area button {
  padding: 0.5rem;
  border-radius: 8px;
  background-color: var(--color-theme);
  color: var(--color-dark);
  display: flex;
  align-items: center;
  justify-content: center;
}

.input-area button:disabled {
  opacity: 0.5;
}

.event-suggestion {
  background-color: rgba(0, 0, 0, 0.05);
  border-radius: 6px;
  padding: 0.75rem;
  margin-bottom: 0.75rem;
}

.event-suggestion h4 {
  margin-top: 0;
  margin-bottom: 0.5rem;
  color: var(--color-theme);
}

.event-details {
  margin-bottom: 0.75rem;
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
  border:none;
  cursor: pointer;
  color: green;
}

.reject-btn {
  background-color: rgba(255, 0, 0, 0.2);
  border:none;
  cursor: pointer;
  color: darkred;
}

/* Typing indicator animation */
.typing-indicator {
  display: flex;
  align-items: center;
  align-self: flex-start;
  background-color: rgba(0, 0, 0, 0.1);
  border-radius: 10px;
  padding: 10px 15px;
}

.typing-indicator span {
  height: 8px;
  width: 8px;
  margin: 0 2px;
  background-color: var(--color-text);
  border-radius: 50%;
  display: inline-block;
  opacity: 0.4;
}

.typing-indicator span:nth-child(1) {
  animation: pulse 1.5s infinite;
}

.typing-indicator span:nth-child(2) {
  animation: pulse 1.5s infinite 0.4s;
}

.typing-indicator span:nth-child(3) {
  animation: pulse 1.5s infinite 0.8s;
}

@keyframes pulse {
  0%, 100% {
    opacity: 0.4;
    transform: scale(1);
  }
  50% {
    opacity: 1;
    transform: scale(1.2);
  }
}

.chat-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 0.5rem 1rem;
  background-color: var(--color-theme);
  border-top-left-radius: 8px;
  border-top-right-radius: 8px;
  color: var(--color-dark);
}

.chat-header h2 {
  margin: 0;
  font-size: 1.2rem;
}

.clear-btn {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 0.25rem 0.5rem;
  border-radius: 10px;
  border: 1px solid var(--color-border);
  background-color: rgba(255, 255, 255, 0.2);
  color: var(--color-dark);
  font-size: 0.9rem;
  cursor: pointer;
}

.clear-btn:hover {
  background-color: rgba(255, 255, 255, 0.3);
}

.clear-btn:active {
  transform: scale(0.9);
}
</style>