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
      <div v-for="(message, index) in chatHistory" :key="index" 
           :class="['message', message.sender === 'user' ? 'user-message' : 'assistant-message']">
        <div v-if="message.sender === 'assistant' && message.eventSuggestion" class="event-suggestion">
          <h4>Event Suggestion:</h4>
          <div class="event-details">
            <p><strong>Description:</strong> {{ message.eventSuggestion.description }}</p>
            <p v-if="message.eventSuggestion.time">
              <strong>Time:</strong> {{ formatDate(message.eventSuggestion.time) }}
            </p>
            <p v-if="message.eventSuggestion.recurrence">
              <strong>Recurrence:</strong> {{ formatRecurrence(message.eventSuggestion.recurrence) }}
            </p>
            <p><strong>Set Alarm:</strong> {{ message.eventSuggestion.alarm ? 'Yes' : 'No' }}</p>
          </div>
          <div class="suggestion-actions" v-if="!message.eventAccepted && !message.eventRejected">
            <button @click="acceptSuggestion(index)" class="accept-btn">Accept</button>
            <button @click="rejectSuggestion(index)" class="reject-btn">Reject</button>
          </div>
          <div v-else-if="message.eventAccepted" class="suggestion-status accepted">
            Event added to calendar
          </div>
          <div v-else class="suggestion-status rejected">
            Event rejected
          </div>
        </div>
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
          <path d="M3 13h6v-2H3V1.846a.5.5 0 0 1 .741-.438l18.462 10.154a.5.5 0 0 1 0 .876L3.741 22.592A.5.5 0 0 1 3 22.154V13z" fill="currentColor"/>
        </svg>
      </button>
    </section>
  </section>
</template>

<script setup lang="ts">
import { ref, onMounted, nextTick, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { format } from 'date-fns'

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

// Format date for display
const formatDate = (dateString: string) => {
  try {
    const date = new Date(dateString);
    return format(date, 'EEEE, MMMM d, yyyy \'at\' h:mm a');
  } catch (e) {
    return dateString;
  }
};

// Format recurrence rule for human reading
const formatRecurrence = (recurrence: string) => {
  if (!recurrence) return '';
  
  if (recurrence.includes('FREQ=DAILY')) {
    const count = recurrence.match(/COUNT=(\d+)/);
    return count ? `Daily (${count[1]} times)` : 'Daily';
  } else if (recurrence.includes('FREQ=WEEKLY')) {
    const count = recurrence.match(/COUNT=(\d+)/);
    return count ? `Weekly (${count[1]} times)` : 'Weekly';
  } else if (recurrence.includes('FREQ=MONTHLY')) {
    const count = recurrence.match(/COUNT=(\d+)/);
    return count ? `Monthly (${count[1]} times)` : 'Monthly';
  } else if (recurrence.includes('FREQ=YEARLY')) {
    const count = recurrence.match(/COUNT=(\d+)/);
    return count ? `Yearly (${count[1]} times)` : 'Yearly';
  }
  return recurrence;
};

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
  const message = userInput.value.trim();
  if (!message || isProcessing.value) return;
  
  // Add user message to chat
  chatHistory.value.push({
    content: message,
    sender: 'user'
  });
  userInput.value = '';
  isProcessing.value = true;
  isTyping.value = true;
  
  await scrollToBottom();
  
  try {
    // Convert chat history to a format suitable for the backend
    const conversation_history = chatHistory.value.map(msg => ({
      role: msg.sender,
      content: msg.content,
      timestamp: new Date().toISOString(),
    }));

    console.log('Sending conversationHistory to AI:', conversation_history);
    
    // Process through our service with conversation history
    const response = await invoke<string>('process_ai_message', { 
      query: message,
      conversationHistory: JSON.stringify(conversation_history)
    });
    
    const parsedResponse = JSON.parse(response);
    
    // Add AI response to chat
    isTyping.value = false;
    
    // Rest of your code remains the same
    const actionTaken = parsedResponse.action_taken || 'none';
    
    if (actionTaken === 'create_event' && 
        parsedResponse.extracted_events && 
        parsedResponse.extracted_events.length > 0) {
      // Add response with event suggestion
      chatHistory.value.push({
        content: parsedResponse.response_text,
        sender: 'assistant',
        eventSuggestion: parsedResponse.extracted_events[0]
      });
    } else if (actionTaken === 'update_event' && 
              parsedResponse.extracted_events && 
              parsedResponse.extracted_events.length > 0) {
      // Handle updating an event
      chatHistory.value.push({
        content: parsedResponse.response_text,
        sender: 'assistant',
        eventSuggestion: parsedResponse.extracted_events[0]
      });
    } else if (actionTaken === 'query_events') {
      // Just show the response text for queries
      chatHistory.value.push({
        content: parsedResponse.response_text,
        sender: 'assistant'
      });
    } else {
      // For 'none' or any other action, just show response text
      chatHistory.value.push({
        content: parsedResponse.response_text,
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

// Accept event suggestion
const acceptSuggestion = async (messageIndex: number) => {
  const message = chatHistory.value[messageIndex];
  if (!message || !message.eventSuggestion) return;
  
  try {
    // Create a calendar event object
    const eventDate = message.eventSuggestion.time ? 
      new Date(message.eventSuggestion.time) : 
      new Date(Date.now() + 60 * 60 * 1000); // Default to 1 hour from now
    
    const calendarEvent = {
      id: crypto.randomUUID(),
      user_id: "",
      description: message.eventSuggestion.description,
      time: eventDate.toISOString(),
      alarm: message.eventSuggestion.alarm,
      synced: false,
      deleted: false,
      recurrence: message.eventSuggestion.recurrence
    };
    
    // Save the event
    await invoke('save_event', { 
      event: JSON.stringify(calendarEvent)
    });
    
    // Mark as accepted
    message.eventAccepted = true;
    
    // Trigger sync
    try {
      await invoke('trigger_sync');
    } catch (syncError) {
      console.warn('Failed to trigger sync:', syncError);
    }
    
    // Schedule notification if alarm is on
    if (calendarEvent.alarm) {
      try {
        await invoke('schedule_event_notification', { 
          event_json: JSON.stringify(calendarEvent) 
        });
      } catch (notificationError) {
        console.warn('Failed to schedule notification:', notificationError);
      }
    }
    
    // Add confirmation message
    chatHistory.value.push({
      content: `Great! I've added "${message.eventSuggestion.description}" to your calendar.`,
      sender: 'assistant'
    });
  } catch (error) {
    console.error('Error saving event:', error);
    chatHistory.value.push({
      content: "I'm sorry, I couldn't save this event to your calendar.",
      sender: 'assistant'
    });
  }
  
  await scrollToBottom();
};

// Reject event suggestion
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
  overflow: hidden;
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
  color: green;
}

.reject-btn {
  background-color: rgba(255, 0, 0, 0.2);
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