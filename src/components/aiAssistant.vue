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
        <div class="message-timestamp">{{ formatTimestamp(message.timestamp) }}</div>
        <DeleteSuggestion
          v-if="message.sender === 'assistant' && message.eventSuggestion && message.isDelete"
          :eventSuggestion="message.eventSuggestion"
          :eventAccepted="message.eventAccepted"
          :eventRejected="message.eventRejected"
          @accept="acceptSuggestion(index)"
          @reject="rejectSuggestion(index)"
        />
        <EventSuggestion
          v-else-if="message.sender === 'assistant' && message.eventSuggestion"
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
import EventSuggestion from './eventSuggestion.vue'
import DeleteSuggestion from './deleteSuggestion.vue'
import ConflictMessage from './conflictMessage.vue'
import { tr } from 'date-fns/locale'

interface EventSuggestion {
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
    sender: 'assistant',
    timestamp: new Date().toISOString()
  }];
};

// Scroll to bottom of chat //
const scrollToBottom = async () => {
  await nextTick();
  if (chatContainer.value) {
    chatContainer.value.scrollTop = chatContainer.value.scrollHeight;
  }
};

// Format timestamp for display //
const formatTimestamp = (timestamp: string) => {
  const date = new Date(timestamp);
  return date.toLocaleTimeString([], { 
    hour: '2-digit', 
    minute: '2-digit',
    hour12: true 
  });
};

const processQuery = async (message: string, isRetry = false) => {
  try {
    let retryCount = 0;
    const maxRetries = 1;

    const attemptQuery = async (msg: string, retry = false): Promise<any> => {
      const promptText = retry 
        ? `Please format your response correctly as a JSON object with response_text, extracted_events, and action_taken fields. Original query: ${msg}` 
        : msg;
        
      const response = await invoke<string>('process_ai_message', { 
        query: promptText,
        conversationHistory: JSON.stringify(chatHistory.value.slice(0, -1))
      });
      
      try {
        const aiResponse = JSON.parse(response);
        
        // Validate response format
        if (!aiResponse.response_text || 
            (aiResponse.action_taken !== 'none' && !aiResponse.extracted_events)) {
          if (retryCount < maxRetries) {
            retryCount++;
            console.log("Invalid AI response format, retrying...");
            return await attemptQuery(msg, true);
          }
          // If we've exhausted retries, try to fix the response here
          return {
            response_text: aiResponse.response_text || "I'm sorry, I couldn't process that correctly.",
            action_taken: aiResponse.action_taken || "none",
            extracted_events: aiResponse.extracted_events || []
          };
        }
        
        return aiResponse;
      } catch (parseError) {
        if (retryCount < maxRetries) {
          retryCount++;
          console.log("Failed to parse AI response, retrying...");
          return await attemptQuery(msg, true);
        }
        throw parseError;
      }
    };
    
    return await attemptQuery(message, isRetry);
  } catch (error) {
    console.error("Error in processQuery:", error);
    // Return a fallback response that won't break the app
    return {
      response_text: "I'm sorry, I encountered an error processing your request.",
      action_taken: "none",
      extracted_events: []
    };
  }
};

// Send message to AI //
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
    
    // Process by action type
    if (aiResponse.extracted_events && aiResponse.extracted_events.length > 0) {
  const event = aiResponse.extracted_events[0];
  
  switch (aiResponse.action_taken) {
    case 'create_event':
    // Use EventSuggestion component for creation events
    chatHistory.value.push({
      content: aiResponse.response_text,
      sender: 'assistant',
      timestamp: new Date().toISOString(),
      eventSuggestion: event
    });
    break;
    
    case 'delete_event':
      // Use DeleteSuggestion component for deletion events
      chatHistory.value.push({
        content: aiResponse.response_text,
        sender: 'assistant',
        timestamp: new Date().toISOString(),
        eventSuggestion: event,
        isDelete: true 
      });
      break;
      
    case 'move_event':
      // Use EventSuggestion with isMoved flag for move operations
      chatHistory.value.push({
        content: aiResponse.response_text,
        sender: 'assistant',
        timestamp: new Date().toISOString(),
        eventSuggestion: event,
        isMoved: true
      });
      break;
      
    case 'update_event':
      // Use EventSuggestion with isUpdate flag for updates
      chatHistory.value.push({
        content: aiResponse.response_text,
        sender: 'assistant',
        timestamp: new Date().toISOString(),
        eventSuggestion: event,
        isUpdate: true 
      });
      break;
          
        default:
          // Just display the response for other action types
          chatHistory.value.push({
            content: aiResponse.response_text,
            sender: 'assistant',
            timestamp: new Date().toISOString()
          });
      }
    } else {
      // No events, just show the response
      chatHistory.value.push({
        content: aiResponse.response_text,
        sender: 'assistant',
        timestamp: new Date().toISOString()
      });
    }
  } catch (error) {
    isTyping.value = false;
    console.error('Error processing message:', error);
    chatHistory.value.push({
      content: "I'm sorry, I encountered an error processing your request. Please try again with more specific details.",
      sender: 'assistant',
      timestamp: new Date().toISOString()
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

// Handle conflict resolution //
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
      sender: 'assistant',
      timestamp: new Date().toISOString()
    });
    
    // Emit event to refresh calendar
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

// Handle conflict cancel //
const handleConflictCancel = async () => {
  const { messageIndex } = conflictData.value;
  
  const message = chatHistory.value[messageIndex];
  if (message) {
    message.eventRejected = true;
  }
  
  chatHistory.value.push({
    content: "No problem. The existing event remains unchanged. Is there anything else you'd like me to help you with?",
    sender: 'assistant',
    timestamp: new Date().toISOString()
  });
  
  showConflictDialog.value = false;
  await scrollToBottom();
};

// Modified acceptSuggestion function
const acceptSuggestion = async (messageIndex: number) => {
  const message = chatHistory.value[messageIndex];
  if (!message || !message.eventSuggestion) return;
  
  try {
    if (message.isDelete) {
      // When user confirms deletion via the UI component
      const eventsJson = await invoke<string[]>('get_events_for_ai');
      const events = eventsJson.map(eventStr => JSON.parse(eventStr));
      
      // Find the matching event again
      const matchingEvent = findMatchingEvent(events, message.eventSuggestion);
      
      if (matchingEvent) {
        await invoke('delete_event', { id: matchingEvent.id });
        message.eventAccepted = true;
        
        // Ensure calendar refreshes
        try {
          await invoke('trigger_sync');
          await tauriEmit('event-saved');
        } catch (syncError) {
          console.warn('Failed to trigger sync:', syncError);
        }
        
        // Add confirmation message
        chatHistory.value.push({
          content: `I've deleted "${matchingEvent.description}" from your calendar.`,
          sender: 'assistant',
          timestamp: new Date().toISOString()
        });
      }
    } else if (message.isUpdate || message.isMoved) {
      await updateExistingEvent(message.eventSuggestion, message.isMoved);
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
      sender: 'assistant',
      timestamp: new Date().toISOString()
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
    sender: 'assistant',
    timestamp: new Date().toISOString()
  });
};

// Function to update an existing event //
const updateExistingEvent = async (eventSuggestion: EventSuggestion, isMoved: boolean = false) => {
  try {
    // Get all events to find the one to update
    const eventsJson = await invoke<string[]>('get_events_for_ai');
    const events = eventsJson.map(eventStr => JSON.parse(eventStr));
    
    // For move operations, find by description and date (not exact time)
    let matchingEvent;
    if (isMoved) {
      matchingEvent = findEventForMove(events, eventSuggestion);
    } else {
      matchingEvent = findMatchingEvent(events, eventSuggestion);
    }
    
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
      sender: 'assistant',
      timestamp: new Date().toISOString()
    });
    
  } catch (error) {
    console.error('Error updating event:', error);
    throw error;
  }
};

// Helper function to find matching event for move operations //
const findEventForMove = (events: any[], eventSuggestion: EventSuggestion) => {
  const targetDescription = eventSuggestion.description.toLowerCase().trim();
  
  // For move operations, we want to find by description primarily
  // and if time is provided, use it to find events on the same day
  if (eventSuggestion.time) {
    const targetTime = new Date(eventSuggestion.time);
    const targetDate = new Date(targetTime.getFullYear(), targetTime.getMonth(), targetTime.getDate());
    
    // Find events on the same day with matching description
    const sameDayEvents = events.filter(e => {
      const eventTime = new Date(e.time);
      const eventDate = new Date(eventTime.getFullYear(), eventTime.getMonth(), eventTime.getDate());
      return eventDate.getTime() === targetDate.getTime() &&
             e.description.toLowerCase().trim() === targetDescription;
    });
    
    if (sameDayEvents.length > 0) {
      return sameDayEvents[0]; // Return the first match
    }
    
    // If no exact description match, try partial match on same day
    const partialMatches = events.filter(e => {
      const eventTime = new Date(e.time);
      const eventDate = new Date(eventTime.getFullYear(), eventTime.getMonth(), eventTime.getDate());
      return eventDate.getTime() === targetDate.getTime() &&
             (e.description.toLowerCase().includes(targetDescription) ||
              targetDescription.includes(e.description.toLowerCase()));
    });
    
    if (partialMatches.length > 0) {
      return partialMatches[0];
    }
  }
  
  // Fallback to description-only matching
  return events.find(e => 
    e.description.toLowerCase().trim() === targetDescription ||
    e.description.toLowerCase().includes(targetDescription) ||
    targetDescription.includes(e.description.toLowerCase())
  );
};

// Function to delete an existing event //
const deleteExistingEvent = async (eventSuggestion: EventSuggestion) => {
  try {
    // Get all events to find the one to delete
    const eventsJson = await invoke<string[]>('get_events_for_ai');
    const events = eventsJson.map(eventStr => JSON.parse(eventStr));
    
    // First prioritize finding events by date
    let matchingEvent = null;
    if (eventSuggestion.time) {
      const deleteTime = new Date(eventSuggestion.time);
      const deleteDay = new Date(deleteTime.getFullYear(), deleteTime.getMonth(), deleteTime.getDate());
      
      // First, try to find an event on the exact same day and time if specified
      const eventsOnSameDay = events.filter(e => {
        const eventTime = new Date(e.time);
        const eventDay = new Date(eventTime.getFullYear(), eventTime.getMonth(), eventTime.getDate());
        return eventDay.getTime() === deleteDay.getTime();
      });
      
      // If there's only one event on that day, use it regardless of time or description
      if (eventsOnSameDay.length === 1) {
        matchingEvent = eventsOnSameDay[0];
      } 
      // If there are multiple events on the same day, try to match by time too
      else if (eventsOnSameDay.length > 1) {
        // Try to match by hour if time was specified
        matchingEvent = eventsOnSameDay.find(e => {
          const eventTime = new Date(e.time);
          return eventTime.getHours() === deleteTime.getHours();
        });
        
        // If still no match, just use the first event of the day
        if (!matchingEvent) {
          matchingEvent = eventsOnSameDay[0];
        }
      }
    }
    
    // If no match by date/time, only then try to find by description as fallback
    if (!matchingEvent) {
      matchingEvent = events.find(e => 
        e.description.toLowerCase().includes(eventSuggestion.description.toLowerCase()) ||
        eventSuggestion.description.toLowerCase().includes(e.description.toLowerCase())
      );
    }
    
    if (!matchingEvent) {
      chatHistory.value.push({
        content: "I couldn't find an event matching your request. Could you provide more details?",
        sender: 'assistant',
        timestamp: new Date().toISOString()
      });
      return;
    }
    
    // Delete the event
    await invoke('delete_event', { 
      id: matchingEvent.id 
    });

    try {
      await invoke('trigger_sync');
      // ADDED: Emit event to refresh calendar UI
      await tauriEmit('event-saved');
    } catch (syncError) {
      console.warn('Failed to trigger sync:', syncError);
    }
    
    // Use the actual event details from the database, not the AI suggestion
    const formattedTime = new Date(matchingEvent.time).toLocaleString();
    chatHistory.value.push({
      content: `I've deleted "${matchingEvent.description}" scheduled for ${formattedTime} from your calendar.`,
      sender: 'assistant',
      timestamp: new Date().toISOString()
    });
    
  } catch (error) {
    console.error('Error deleting event:', error);
    chatHistory.value.push({
      content: "I'm sorry, I encountered an error while trying to delete the event.",
      sender: 'assistant',
      timestamp: new Date().toISOString()
    });
  }
};

const handleEventDeletion = async (eventSuggestion: EventSuggestion, responseText: string) => {
  try {
    const eventsJson = await invoke<string[]>('get_events_for_ai');
    const events = eventsJson.map(eventStr => JSON.parse(eventStr));
    
    // Time-based matching (prioritize)
    let matchingEvent = null;
    if (eventSuggestion.time) {
      const deleteTime = new Date(eventSuggestion.time);
      
      // Match events on the same day
      const eventsOnSameDay = events.filter(e => {
        const eventTime = new Date(e.time);
        return eventTime.getFullYear() === deleteTime.getFullYear() &&
               eventTime.getMonth() === deleteTime.getMonth() &&
               eventTime.getDate() === deleteTime.getDate();
      });
      
      if (eventsOnSameDay.length === 1) {
        // If only one event on that day, it's likely the target
        matchingEvent = eventsOnSameDay[0];
      } else if (eventsOnSameDay.length > 1) {
        // Multiple events on same day, try to match by hour
        matchingEvent = eventsOnSameDay.find(e => {
          const eventTime = new Date(e.time);
          return eventTime.getHours() === deleteTime.getHours();
        });
        
        // If still no match by hour, show suggestion UI for user to select
        if (!matchingEvent) {
          // Show event selection UI with all events from that day
          chatHistory.value.push({
            content: responseText,
            sender: 'assistant',
            timestamp: new Date().toISOString(),
            eventSuggestion: eventSuggestion,
            isDelete: true,
            multipleOptions: eventsOnSameDay // Add this field to store multiple options
          });
          return;
        }
      }
    }
    
    // If no match by time, try description
    if (!matchingEvent) {
      matchingEvent = events.find(e => 
        e.description.toLowerCase().includes(eventSuggestion.description.toLowerCase()) ||
        eventSuggestion.description.toLowerCase().includes(e.description.toLowerCase())
      );
    }
    
    if (!matchingEvent) {
      chatHistory.value.push({
        content: "I couldn't find an event matching your request. Could you please provide more specific details like the date or time?",
        sender: 'assistant',
        timestamp: new Date().toISOString()
      });
      return;
    }
    
    // Delete the found event
    await invoke('delete_event', { id: matchingEvent.id });
    await invoke('trigger_sync').catch(e => console.warn('Failed to trigger sync:', e));
    
    // Show confirmation with actual event details
    const formattedTime = new Date(matchingEvent.time).toLocaleString();
    chatHistory.value.push({
      content: `I've deleted "${matchingEvent.description}" scheduled for ${formattedTime} from your calendar.`,
      sender: 'assistant',
      timestamp: new Date().toISOString()
    });
  } catch (error) {
    console.error('Error deleting event:', error);
    chatHistory.value.push({
      content: "I encountered an error while trying to delete the event.",
      sender: 'assistant',
      timestamp: new Date().toISOString()
    });
  }
};

// Function to delete all events //
const deleteAllEvents = async () => {
  try {
    const deletedCount = await invoke<number>('delete_all_events');
    
    try {
      await invoke('trigger_sync');
    } catch (syncError) {
      console.warn('Failed to trigger sync:', syncError);
    }
    
    chatHistory.value.push({
      content: `I've deleted all your events from the calendar.`,
      sender: 'assistant',
      timestamp: new Date().toISOString()
    });
    
  } catch (error) {
    console.error('Error deleting all events:', error);
    chatHistory.value.push({
      content: "I'm sorry, I encountered an error while trying to delete your events.",
      sender: 'assistant',
      timestamp: new Date().toISOString()
    });
  }
};

// Helper function to find matching event //
const findMatchingEvent = (events: any[], eventSuggestion: EventSuggestion) => {
  if (eventSuggestion.time) {
    const targetTime = new Date(eventSuggestion.time);
    
    // 1. Try exact time match first (same hour on same day)
    let match = events.find(e => {
      const eventTime = new Date(e.time);
      return eventTime.getFullYear() === targetTime.getFullYear() &&
             eventTime.getMonth() === targetTime.getMonth() &&
             eventTime.getDate() === targetTime.getDate() &&
             eventTime.getHours() === targetTime.getHours();
    });
    
    if (match) return match;
    
    // 2. Try same day match
    const sameDayEvents = events.filter(e => {
      const eventTime = new Date(e.time);
      return eventTime.getFullYear() === targetTime.getFullYear() &&
             eventTime.getMonth() === targetTime.getMonth() &&
             eventTime.getDate() === targetTime.getDate();
    });
    
    if (sameDayEvents.length === 1) return sameDayEvents[0];
    
    // 3. If multiple events on same day, try partial description match
    if (sameDayEvents.length > 1) {
      const description = eventSuggestion.description.toLowerCase();
      match = sameDayEvents.find(e => 
        e.description.toLowerCase().includes(description) || 
        description.includes(e.description.toLowerCase())
      );
      
      if (match) return match;
    }
  }
  
  // 4. If no time match or no time provided, fall back to description match
  const description = eventSuggestion.description.toLowerCase();
  return events.find(e => e.description.toLowerCase() === description ||
                          e.description.toLowerCase().includes(description) ||
                          description.includes(e.description.toLowerCase()));
};

// Function to find events in a date range //
const findEventsInDateRange = (events: any[], startDate: Date, endDate: Date | null = null) => {
  // If no end date, assume same day as start date
  const rangeEndDate = endDate || new Date(
    startDate.getFullYear(),
    startDate.getMonth(),
    startDate.getDate(),
    23, 59, 59
  );
  
  return events.filter(e => {
    const eventTime = new Date(e.time);
    return eventTime >= startDate && eventTime <= rangeEndDate;
  });
};

// Enhanced delete function for date ranges
const deleteEventsInRange = async (startDate: Date, endDate: Date | null = null) => {
  try {
    const eventsJson = await invoke<string[]>('get_events_for_ai');
    const events = eventsJson.map(eventStr => JSON.parse(eventStr));
    
    const eventsToDelete = findEventsInDateRange(events, startDate, endDate);
    
    if (eventsToDelete.length === 0) {
      chatHistory.value.push({
        content: "I couldn't find any events in the specified date range.",
        sender: 'assistant',
        timestamp: new Date().toISOString()
      });
      return;
    }
    
    // Delete each event
    for (const event of eventsToDelete) {
      await invoke('delete_event', { id: event.id });
    }
    
    // Trigger sync after all deletions
    await invoke('trigger_sync').catch(e => console.warn('Failed to trigger sync:', e));
    
    // Build a meaningful response based on date range and deleted count
    const formatDate = (date: Date) => date.toLocaleDateString();
    let responseMsg = '';
    
    if (endDate) {
      responseMsg = `I've deleted ${eventsToDelete.length} events from ${formatDate(startDate)} to ${formatDate(endDate)}.`;
    } else {
      responseMsg = `I've deleted ${eventsToDelete.length} events on ${formatDate(startDate)}.`;
    }
    
    chatHistory.value.push({
      content: responseMsg,
      sender: 'assistant',
      timestamp: new Date().toISOString()
    });
    
  } catch (error) {
    console.error('Error deleting events in range:', error);
    chatHistory.value.push({
      content: "I encountered an error while trying to delete events in that date range.",
      sender: 'assistant',
      timestamp: new Date().toISOString()
    });
  }
};

// Reject event suggestion //
const rejectSuggestion = async (messageIndex: number) => {
  const message = chatHistory.value[messageIndex];
  if (!message) return;
  
  message.eventRejected = true;
  
  chatHistory.value.push({
    content: "No problem. Is there anything else you'd like me to help you with?",
    sender: 'assistant',
    timestamp: new Date().toISOString()
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