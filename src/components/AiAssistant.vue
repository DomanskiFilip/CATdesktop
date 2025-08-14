<template>
  <section id="ai-assistant">
    <!-- Chat clear button -->
    <div id="chat-header">
      <h2>Calendar Assistant</h2>
      <button @click="clearChat" class="clear-btn" title="Clear conversation">
        <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="16" height="16">
          <path fill="none" d="M0 0h24v24H0z"/>
          <path d="M17 6h5v2h-2v13a1 1 0 01-1 1H5a1 1 0 01-1-1V8H2V6h5V3a1 1 0 011-1h8a1 1 0 011 1v3zm1 2H6v12h12V8zm-9 3h2v6H9v-6zm4 0h2v6h-2v-6zM9 4v2h6V4H9z" fill="currentColor"/>
        </svg>
        Clear Chat
      </button>
    </div>
    <!-- Chat -->
    <section id="chat-container" ref="chatContainer">
      <div v-for="(message, index) in chatHistory" :key="index" :class="['message', message.sender === 'user' ? 'user-message' : 'assistant-message']">
        <div class="message-timestamp">{{ formatTimestamp(message.timestamp) }}</div>
        <DeleteSuggestion v-if="message.sender === 'assistant' && message.eventSuggestion && message.isDelete" :eventSuggestion="message.eventSuggestion" :eventAccepted="message.eventAccepted" :eventRejected="message.eventRejected" @accept="acceptSuggestion(index)" @reject="rejectSuggestion(index)"/>
        <EventSuggestion v-else-if="message.sender === 'assistant' && message.eventSuggestion" :eventSuggestion="message.eventSuggestion" :eventAccepted="message.eventAccepted" :eventRejected="message.eventRejected" :isUpdate="message.isUpdate" :isMoved="message.isMoved" @accept="acceptSuggestion(index)" @reject="rejectSuggestion(index)"/>
        <div class="message-content">{{ message.content }}</div>
      </div>
      <div v-if="isTyping" class="typing-indicator">
        <span></span>
        <span></span>
        <span></span>
      </div>
    </section>
    <!-- Suggestions -->
    <section id="suggestions-area">
      <button class="scroll-btn left" @click="scrollSuggestions('left')" aria-label="Scroll left">
        <svg xmlns="http://www.w3.org/2000/svg" height="20px" viewBox="0 -960 960 960" width="20px" fill="var(--color-text)"><path d="M624-96 240-480l384-384 68 68-316 316 316 316-68 68Z"/></svg>
      </button>
      <ul ref="suggestionsList">
        <li @click="useSuggestion($event)">shedule an event at:</li>
        <li @click="useSuggestion($event)">delete an event at:</li>
        <li @click="useSuggestion($event)">move event at:</li>
        <li @click="useSuggestion($event)">change the description of an event at:</li>
        <li @click="useSuggestion($event)">what kind of a cat are you?</li>
      </ul>
      <button class="scroll-btn right" @click="scrollSuggestions('right')" aria-label="Scroll right">
       <svg xmlns="http://www.w3.org/2000/svg" height="20px" viewBox="0 -960 960 960" width="20px" fill="var(--color-text)"><path d="m288-96-68-68 316-316-316-316 68-68 384 384L288-96Z"/></svg>
      </button>
    </section>
    
    <section id="input-area">
      <input  type="text" v-model="userInput" @keyup.enter="sendMessage" placeholder="Ask me to create events, check your schedule, etc." :disabled="isProcessing"/>
      <SpeechToText @transcription="handleTranscription" @error="handleSpeechError"/>
      <button @click="sendMessage" :disabled="isProcessing || !userInput.trim()">
        <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="24" height="24">
          <path fill="none" d="M0 0h24v24H0z"/>
          <path d="M3 13h6v-2H3V1.846a.5.5 0 0 1 .741-.438l18.462 10.154a.5.5 0 0 1 0 .876L3.741 22.592A.5.5 0 0 1 3 22.154V13z" fill="var(--color-dark)"/>
        </svg>
      </button>
    </section>

    <!-- Conflict Resolution Dialog -->
    <ConflictMessage :isVisible="showConflictDialog" :existingEvent="conflictData.existingEvent" :newSuggestion="conflictData.newSuggestion" :conflictType="conflictData.conflictType" @update="handleConflictUpdate" @cancel="handleConflictCancel"/>
  </section>
</template>

<script setup lang="ts">
import { ref, onMounted, nextTick, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { emit as tauriEmit } from '@tauri-apps/api/event'
import DeleteSuggestion from './DeleteSuggestion.vue'
import EventSuggestion from './EventSuggestion.vue'
import ConflictMessage from './ConflictMessage.vue'
import SpeechToText from './SpeechToText.vue'

interface EventSuggestion {
  target_event_id?: string;
  description: string;
  time?: string;
  alarm: boolean;
  recurrence?: string;
}

interface ChatMessage {
  content: string;
  sender: 'user' | 'assistant';
  timestamp: string;
  eventSuggestion?: EventSuggestion;
  eventAccepted?: boolean;
  eventRejected?: boolean;
  isUpdate?: boolean;
  isMoved?: boolean;
  isDelete?: boolean;
  multipleOptions?: any[];
}

const chatHistory = ref<ChatMessage[]>([
  {
    content: "Hello! I'm your Calendar AssistanT, You can call me CAT. How can I help you manage your schedule today?",
    sender: 'assistant',
    timestamp: new Date().toISOString()
  }
]);
const userInput = ref('');
const isProcessing = ref(false);
const isTyping = ref(false);
const chatContainer = ref<HTMLElement | null>(null);
const suggestionsList = ref<HTMLElement | null>(null);

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

// == Utility functions == //
// Utility function -> clear chat history
const clearChat = () => {
  chatHistory.value = [{
    content: "Hello! I'm your Calendar AssistanT, You can call me CAT. How can I help you manage your schedule today?",
    sender: 'assistant',
    timestamp: new Date().toISOString()
  }];
};

// Utility function -> scroll to bottom of chat
const scrollToBottom = async () => {
  await nextTick();
  if (chatContainer.value) {
    chatContainer.value.scrollTop = chatContainer.value.scrollHeight;
  }
};

// Format timestamp for display
const formatTimestamp = (timestamp: string) => {
  const date = new Date(timestamp);
  return date.toLocaleTimeString([], { 
    hour: '2-digit', 
    minute: '2-digit',
    hour12: true 
  });
};

// Utility function -> use suggestion text from click event or first li
const useSuggestion = (event?: MouseEvent) => {
  // If called from click, event will be present
  let suggestionText = '';
  if (event && event.target instanceof HTMLElement) {
    suggestionText = event.target.innerText;
  } else {
    // fallback: get first li
    const suggestion = document.querySelector('#suggestions-area li');
    if (suggestion) {
      suggestionText = suggestion.textContent || '';
    }
  }
  userInput.value = suggestionText;
};

// arrows to scroll suggestions
const scrollSuggestions = (direction: 'left' | 'right') => {
  const ul = suggestionsList.value;
  if (!ul) return;
  const scrollAmount = 120;
  if (direction === 'left') {
    ul.scrollBy({ left: -scrollAmount, behavior: 'smooth' });
  } else {
    ul.scrollBy({ left: scrollAmount, behavior: 'smooth' });
  }
};


// == Main AI tool logic == //
// Process user query and get AI response
const processQuery = async (message: string) => {
  try {
    const response = await invoke<string>('process_ai_message', { 
      query: message,
      conversationHistory: JSON.stringify(chatHistory.value.slice(0, -1))
    });
    const aiResponse = JSON.parse(response);
    if (!aiResponse.response_text || !aiResponse.action_taken) {
      throw new Error('Invalid AI response format');
    }
    return aiResponse;
  } catch (error) {
    console.error("Error in processQuery:", error);

    // Check if the error message starts with the rate limit emoji
    const errorString = error instanceof Error ? error.message : String(error);
    if (errorString.startsWith('🚫')) {
      return {
        response_text: errorString,
        action_taken: "none",
        extracted_events: [],
        confidence: 0.0
      };
    }
    
    return {
      response_text: "I'm sorry, I encountered an error processing your request.",
      action_taken: "none",
      extracted_events: [],
      confidence: 0.0
    };
  }
};

// Send message to AI handler
const sendMessage = async () => {
  if (!userInput.value.trim() || isProcessing.value) return;
  const message = userInput.value.trim();
  userInput.value = '';
  chatHistory.value.push({
    content: message,
    sender: 'user',
    timestamp: new Date().toISOString()
  });
  isProcessing.value = true;
  isTyping.value = true;
  await scrollToBottom();
  try {
    const aiResponse = await processQuery(message);
    isTyping.value = false;
    await handleAIResponse(aiResponse);
  } catch (error) {
    isTyping.value = false;
    console.error('Error processing message:', error);
    chatHistory.value.push({
      content: "I'm sorry, I encountered an error processing your request.",
      sender: 'assistant',
      timestamp: new Date().toISOString()
    });
  } finally {
    isProcessing.value = false;
    await scrollToBottom();
  }
};

// Unified handler for AI responses filters what suggestion component to use based on action_taken
const handleAIResponse = async (aiResponse: any) => {
  const baseMessage = {
    content: aiResponse.response_text,
    sender: 'assistant' as const,
    timestamp: new Date().toISOString()
  };

  switch (aiResponse.action_taken) {
    // handle create event tool use
    case 'create_event':
      if (aiResponse.extracted_events && aiResponse.extracted_events.length > 0) {
        const suggestion = aiResponse.extracted_events[0];
        const conflict = await checkForConflicts(suggestion);
        if (conflict) {
          conflictData.value = {
            existingEvent: conflict.existingEvent,
            newSuggestion: suggestion,
            conflictType: conflict.type,
            messageIndex: chatHistory.value.length
          };
          chatHistory.value.push({
            ...baseMessage,
            content: baseMessage.content + " However, I found a potential conflict."
          });
          showConflictDialog.value = true;
        } else {
          chatHistory.value.push({
            ...baseMessage,
            eventSuggestion: suggestion
          });
        }
      } else {
        chatHistory.value.push(baseMessage);
      }
      break;

    // handle update event tool use
    case 'update_event':
      if (aiResponse.extracted_events && aiResponse.extracted_events.length > 0) {
        const event = aiResponse.extracted_events[0];
        if (event.target_event_id) {
          chatHistory.value.push({
            ...baseMessage,
            eventSuggestion: event,
            isUpdate: true
          });
        } else {
          chatHistory.value.push({
            ...baseMessage,
            content: baseMessage.content + " (Note: I couldn't identify which specific event to update)"
          });
        }
      } else {
        chatHistory.value.push(baseMessage);
      }
      break;

    // handle delete event tool use
    case 'delete_event':
      if (aiResponse.extracted_events && aiResponse.extracted_events.length > 0) {
        const event = aiResponse.extracted_events[0];
        if (event.target_event_id) {
          chatHistory.value.push({
            ...baseMessage,
            eventSuggestion: event,
            isDelete: true
          });
        } else {
          chatHistory.value.push({
            ...baseMessage,
            content: baseMessage.content + " (Note: I couldn't identify which specific event to delete)"
          });
        }
      } else {
        chatHistory.value.push(baseMessage);
      }
      break;

    case 'none':
    default:
      chatHistory.value.push(baseMessage);
      break;
  }
};

// Check for conflicts when creating events
const checkForConflicts = async (suggestion: EventSuggestion) => {
  try {
    const eventsJson = await invoke<string[]>('get_events');
    const events = eventsJson.map(eventStr => JSON.parse(eventStr));
    if (suggestion.time) {
      const suggestionTime = new Date(suggestion.time);
      const timeConflict = events.find(e => {
        const eventTime = new Date(e.time);
        return Math.abs(eventTime.getTime() - suggestionTime.getTime()) < 30 * 60 * 1000;
      });
      if (timeConflict) {
        return { existingEvent: timeConflict, type: 'time' };
      }
    }
    const descriptionConflict = events.find(e => 
      e.description.toLowerCase().trim() === suggestion.description.toLowerCase().trim()
    );
    if (descriptionConflict) {
      return { existingEvent: descriptionConflict, type: 'description' };
    }
    return null;
  } catch (error) {
    console.error('Error checking conflicts:', error);
    return null;
  }
};

// Accept suggestion handler executeing accepted event suggestion
const acceptSuggestion = async (messageIndex: number) => {
  const message = chatHistory.value[messageIndex];
  if (!message || !message.eventSuggestion) return;
  try {
    const suggestion = message.eventSuggestion;
    if (message.isDelete) {
      await handleDeleteEvent(suggestion, message);
    } else if (message.isUpdate) {
      await handleUpdateEvent(suggestion, message);
    } else {
      await createNewEvent(suggestion);
      message.eventAccepted = true;
    }
    try {
      await invoke('trigger_sync');
      await tauriEmit('event-saved');
    } catch (syncError) {
      console.warn('Failed to trigger sync:', syncError);
    }
  } catch (error) {
    console.error('Error processing event suggestion:', error);
    chatHistory.value.push({
      content: "I'm sorry, I couldn't process this event suggestion.",
      sender: 'assistant',
      timestamp: new Date().toISOString()
    });
  }
  await scrollToBottom();
};

// Handle deleting an event by target_event_id
const handleDeleteEvent = async (suggestion: EventSuggestion, message: ChatMessage) => {
  if (suggestion.target_event_id) {
    await invoke('delete_event', { id: suggestion.target_event_id });
    message.eventAccepted = true;
    chatHistory.value.push({
      content: `✅ I've successfully deleted "${suggestion.description}" from your calendar.`,
      sender: 'assistant',
      timestamp: new Date().toISOString()
    });
  } else {
    throw new Error('Could not find the event to delete');
  }
};

// Handle updating an event by target_event_id
const handleUpdateEvent = async (suggestion: EventSuggestion, message: ChatMessage) => {
  if (suggestion.target_event_id) {
    await updateEventByTargetId(suggestion);
    message.eventAccepted = true;
    chatHistory.value.push({
      content: `✅ I've successfully updated "${suggestion.description}" in your calendar.`,
      sender: 'assistant',
      timestamp: new Date().toISOString()
    });
  } else {
    throw new Error("Could not find the event to update");
  }
};

// Update event by target_event_id
const updateEventByTargetId = async (eventSuggestion: EventSuggestion) => {
  if (!eventSuggestion.target_event_id) {
    throw new Error('No target event ID provided');
  }
  const eventsJson = await invoke<string[]>('get_events');
  const events = eventsJson.map(eventStr => JSON.parse(eventStr));
  const existingEvent = events.find(e => e.id === eventSuggestion.target_event_id);
  if (!existingEvent) {
    throw new Error('Target event not found');
  }
  const updatedEvent = {
    ...existingEvent,
    description: eventSuggestion.description || existingEvent.description,
    time: eventSuggestion.time || existingEvent.time,
    alarm: eventSuggestion.alarm !== undefined ? eventSuggestion.alarm : existingEvent.alarm,
    recurrence: eventSuggestion.recurrence || existingEvent.recurrence,
    // Preserve participants and other fields
    participants: existingEvent.participants || [],
    location: existingEvent.location || '',
    // Reset sync flags
    synced: false,
    synced_google: false
  };
  await invoke('save_event', { event: JSON.stringify(updatedEvent) });
  if (updatedEvent.alarm) {
    try {
      await invoke('schedule_event_notification', { eventJson: JSON.stringify(updatedEvent) });
    } catch (notificationError) {
      console.warn('Failed to schedule notification:', notificationError);
    }
  }
};

// Create a new event
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
    synced_outlook: false,
    deleted: false,
    recurrence: eventSuggestion.recurrence,
    participants: [],
    location: ""
  };
  await invoke('save_event', { event: JSON.stringify(calendarEvent) });
  if (calendarEvent.alarm) {
    try {
      await invoke('schedule_event_notification', { event_json: JSON.stringify(calendarEvent) });
    } catch (notificationError) {
      console.warn('Failed to schedule notification:', notificationError);
    }
  }
};

// Handle conflict resolution
const handleConflictUpdate = async () => {
  const { existingEvent, newSuggestion, conflictType } = conflictData.value;
  if (!newSuggestion) {
    console.error('No suggestion data available for update');
    showConflictDialog.value = false;
    return;
  }
  try {
    await updateConflictingEvent(existingEvent, newSuggestion, conflictType);
    chatHistory.value.push({
      content: `✅ I've updated the existing event "${newSuggestion.description}" with your new details.`,
      sender: 'assistant',
      timestamp: new Date().toISOString()
    });
    await tauriEmit('event-saved');
  } catch (error) {
    console.error('Error updating conflicting event:', error);
    chatHistory.value.push({
      content: "I'm sorry, I couldn't update this event.",
      sender: 'assistant',
      timestamp: new Date().toISOString()
    });
  }
  showConflictDialog.value = false;
  await scrollToBottom();
};

const handleConflictCancel = async () => {
  chatHistory.value.push({
    content: "No problem. The existing event remains unchanged. Is there anything else you'd like me to help you with?",
    sender: 'assistant',
    timestamp: new Date().toISOString()
  });
  showConflictDialog.value = false;
  await scrollToBottom();
};

// Update conflicting event by id
const updateConflictingEvent = async (existingEvent: any, newSuggestion: EventSuggestion, conflictType: string) => {
  if (!newSuggestion) throw new Error('No suggestion data provided');
  let updatedEvent;
  if (conflictType === 'time') {
    updatedEvent = {
      ...existingEvent,
      description: newSuggestion.description,
      alarm: newSuggestion.alarm !== undefined ? newSuggestion.alarm : existingEvent.alarm,
      recurrence: newSuggestion.recurrence || existingEvent.recurrence,
      participants: existingEvent.participants || [],
      location: existingEvent.location || '',
      synced: false,
      synced_google: false,
      synced_outlook: false
    };
  } else if (conflictType === 'description') {
    updatedEvent = {
      ...existingEvent,
      time: newSuggestion.time || existingEvent.time,
      alarm: newSuggestion.alarm !== undefined ? newSuggestion.alarm : existingEvent.alarm,
      recurrence: newSuggestion.recurrence || existingEvent.recurrence,
      participants: existingEvent.participants || [],
      location: existingEvent.location || '',
      synced: false,
      synced_google: false,
      synced_outlook: false
    };
  }
  await invoke('save_event', { event: JSON.stringify(updatedEvent) });
  if (updatedEvent.alarm) {
    try {
      await invoke('schedule_event_notification', { event_json: JSON.stringify(updatedEvent) });
    } catch (notificationError) {
      console.warn('Failed to schedule notification:', notificationError);
    }
  }
  try {
    await invoke('trigger_sync');
  } catch (syncError) {
    console.warn('Failed to trigger sync:', syncError);
  }
};

// Reject event suggestion
const rejectSuggestion = async (messageIndex: number) => {
  const message = chatHistory.value[messageIndex];
  if (!message) return;
  message.eventRejected = true;
  let rejectionMessage = "No problem. ";
  if (message.isDelete) {
    rejectionMessage += "The event remains in your calendar. ";
  } else if (message.isUpdate) {
    rejectionMessage += "I've kept the event as it was. ";
  } else {
    rejectionMessage += "I won't add this event to your calendar. ";
  }
  rejectionMessage += "Is there anything else you'd like me to help you with?";
  chatHistory.value.push({
    content: rejectionMessage,
    sender: 'assistant',
    timestamp: new Date().toISOString()
  });
  await scrollToBottom();
};

// Handle speech-to-text transcription //
const handleTranscription = (text: string) => {
  userInput.value = text
  // Focus on input for user to review before sending
  nextTick(() => {
    const inputElement = document.querySelector('#input-area input') as HTMLInputElement
    if (inputElement) {
      inputElement.focus()
      // Place cursor at the end
      inputElement.setSelectionRange(inputElement.value.length, inputElement.value.length)
    }
  })
}

const handleSpeechError = (error: string) => {
  console.error('Speech recognition error:', error)
  
  // Dedicated rate limit handling
  if (error.startsWith('RATE_LIMIT:')) {
    const rateLimitMessage = error.replace('RATE_LIMIT:', '').trim()
    chatHistory.value.push({
      content: rateLimitMessage,
      sender: 'assistant',
      timestamp: new Date().toISOString()
    })
    scrollToBottom()
    return
  }
  
  // Regular error handling
  chatHistory.value.push({
    content: `Speech recognition error: ${error}`,
    sender: 'assistant', 
    timestamp: new Date().toISOString()
  })
  scrollToBottom()
}

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
#ai-assistant {
  display: flex;
  flex-direction: column;
  height: 90vh;
  max-width: 800px;
  margin: 0 auto;
  background-color: var(--color-main);
  border-radius: 8px;
}

#chat-container {
  flex: 1;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 1rem;
  padding: 1rem;
  background-color: rgba(0, 0, 0, 0.05);
  border-radius: 8px;
}

#chat-container::-webkit-scrollbar {
  width: 8px;
  background: transparent;
}

.message-timestamp {
  font-size: 0.75rem;
  color: var(--color-text);
  opacity: 0.6;
  margin-bottom: 0.25rem;
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
  background-color: var(--color-main);
}

#suggestions-area {
  position: relative;
  display: flex;
  align-items: center;
  background-color: rgba(0, 0, 0, 0.05);
  border-radius: 8px;
  margin-top: 0.5rem;
  margin-bottom: 0.5rem;
  max-width: 100%;
  height: 3rem;
  overflow: hidden;
}

#suggestions-area ul {
  list-style: none;
  padding: 0;
  margin: 0;
  display: flex;
  justify-content: flex-start;
  align-items: center;
  gap: 0.5rem;
  overflow-x: auto;
  scroll-behavior: smooth;
  flex: 1;
}

#suggestions-area ul li{
  background-color: var(--color-theme);
  color: var(--color-text);
  padding: 0.1rem;
  padding-left: 0.5rem;
  padding-right: 0.5rem;
  border-radius: 10px;
  cursor: pointer;
  text-wrap: nowrap;
}

#suggestions-area ul li:hover{
  background-color: var(--color-text);
  color: var(--color-theme);
  transition: background-color 0.2s, color 0.2s;
}

#suggestions-area ul::-webkit-scrollbar {
  display: none;
  width: 0 !important;
  height: 0 !important;
  background: transparent !important;
}

#suggestions-area ul::-webkit-scrollbar-thumb {
  display: none;
  width: 0 !important;
  height: 0 !important;
  background: transparent !important;
}

.scroll-btn {
  background: transparent;
  color: var(--color-text);
  border: none;
  border-radius: 10px;
  width: 1.5rem;
  height: 2.5rem;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  text-align: center;
  margin: 0 0.25rem;
  padding: 0;
  z-index: 1;
}

.scroll-btn:hover {
  background-color: var(--color-theme);
  color: var(--color-text);
  transition: background-color 0.2s, color 0.2s;
}

.scroll-btn:hover svg {
  fill: var(--color-text);
}

.scroll-btn.left {
  order: 0;
}

.scroll-btn.right {
  order: 2;
}

#suggestions-area ul {
  order: 1;
  min-width: 0;
  width: 100%;
  touch-action: pan-x;
}

#input-area {
  display: flex;
  gap: 0.5rem;
  margin-top: auto;
}

#input-area input {
  flex: 1;
  padding: 0.75rem;
  border-radius: 8px;
  border: 1px solid rgba(0, 0, 0, 0.2);
  background-color: rgba(0, 0, 0, 0.05);
  color: var(--color-text);
}

#input-area button {
  padding: 0.5rem;
  border-radius: 8px;
  border: none;
  cursor: pointer;
  background-color: var(--color-theme);
  color: var(--color-dark);
  display: flex;
  align-items: center;
  justify-content: center;
}

#input-area button:disabled {
  opacity: 0.5;
  cursor: not-allowed;
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

#chat-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 0.5rem 1rem;
  background-color: var(--color-theme);
  border-top-left-radius: 8px;
  border-top-right-radius: 8px;
  color: var(--color-dark);
}

#chat-header h2 {
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

/* Mobile */
#main-page.is-mobile #main-content .content-section #ai-assistant {
  margin-top: 2rem;
  height: 93%;
}

#main-page.is-mobile #main-content .content-section #ai-assistant #suggestions-area ul{
  touch-action: pan-x;
}

#main-page.is-mobile #main-content .content-section #ai-assistant #suggestions-area ul li {
  flex-shrink: 0;
  height: 2.5rem;
  display: flex;
  align-items: center;
  justify-content: center;
}

#main-page.is-mobile #main-content .content-section #ai-assistant #input-area {
  margin: 0 1rem;
}
</style>
