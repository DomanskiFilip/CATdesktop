<template>
  <div class="mini-ai-chat" v-if="!props.disabled">
    <div class="mini-message" v-if="clarifyingQuestion">
      <span class="ai-label">CAT:</span> {{ clarifyingQuestion }}
      
      <!-- Feedback buttons (identical to AiAssistant) -->
      <div v-if="!feedbackGiven" class="feedback-buttons">
        <button @click="submitFeedback('thumbs_up')" class="feedback-btn thumbs-up" title="Helpful response">
          <svg xmlns="http://www.w3.org/2000/svg" height="16px" viewBox="0 -960 960 960" width="16px" fill="currentColor">
            <path d="M720-120H280v-520l280-280 50 50q7 7 11.5 19t4.5 23v14l-44 174h258q32 0 56 24t24 56v80q0 7-2 15t-4 15L794-168q-9 20-30 34t-44 14Zm-360-80h360l120-280v-80H480l54-220-174 174v406Zm0-406v406-406Zm-80-34v80H160v360h120v80H80v-520h200Z"/>
          </svg>
        </button>
        <button @click="submitFeedback('thumbs_down')" class="feedback-btn thumbs-down" title="Not helpful">
          <svg xmlns="http://www.w3.org/2000/svg" height="16px" viewBox="0 -960 960 960" width="16px" fill="currentColor">
            <path d="M240-840h440v520L400-40l-50-50q-7-7-11.5-19t-4.5-23v-14l44-174H120q-32 0-56-24t-24-56v-80q0-7 2-15t4-15l120-282q9-20 30-34t44-14Zm360 80H240L120-480v80h360l-54 220 174-174v-406Zm0 406v-406 406Zm80 34v-80h120v-360H680v-80h200v520H680Z"/>
          </svg>
        </button>
      </div>
      <div v-if="feedbackGiven" class="feedback-given">
        {{ feedbackType === 'thumbs_up' ? '👍 Thanks for the feedback!' : '👎 Thanks for the feedback!' }}
      </div>
    </div>
    
    <div class="mini-input-area" v-if="!props.disabled">
      <input v-model="userInput" @keyup.enter="sendAnswer" :disabled="isProcessing" placeholder="Type your answer..."/>
      <button @click="sendAnswer" :disabled="isProcessing || !userInput.trim()">Send</button>
    </div>
    
    <div class="mini-message user" v-if="userAnswer">
      <span class="user-label">You:</span> {{ userAnswer }}
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'

const props = defineProps<{
  clarifyingQuestion: string
  disabled?: boolean
  requestType?: string
  confidence?: number
}>()
const emit = defineEmits(['answered'])

const userInput = ref('')
const userAnswer = ref('')
const isProcessing = ref(false)
const feedbackGiven = ref(false)
const feedbackType = ref<'thumbs_up' | 'thumbs_down' | null>(null)

const sendAnswer = () => {
  if (!userInput.value.trim()) return
  userAnswer.value = userInput.value
  isProcessing.value = true
  emit('answered', userInput.value)
}

const submitFeedback = async (type: 'thumbs_up' | 'thumbs_down') => {
  if (feedbackGiven.value) return
  
  try {
    await invoke('submit_feedback', {
      feedbackType: type,
      responseText: props.clarifyingQuestion,
      actionTaken: 'clarification',
      confidence: props.confidence || 0.5,
      originalRequestType: props.requestType || 'event_enrichment',
      hasEventSuggestion: false
    })
    
    feedbackGiven.value = true
    feedbackType.value = type
  } catch (error) {
    console.error('Failed to submit mini chat feedback:', error)
  }
}
</script>

<style scoped>
.mini-ai-chat {
  background: var(--color-main);
  border-radius: 8px;
  padding: 0.5rem;
  box-shadow: 0 2px 8px rgba(0,0,0,0.08);
  font-size: 0.95em;
  width: 100%;
}
.mini-message {
  margin-bottom: 0.5rem;
  color: var(--color-text);
}
.mini-message.user {
  color: var(--color-theme);
}
.ai-label {
  font-weight: bold;
  margin-right: 0.25em;
}
.user-label {
  font-weight: bold;
  margin-right: 0.25em;
}
.mini-input-area {
  display: flex;
  gap: 0.25rem;
}
.mini-input-area input {
  flex: 1;
  border-radius: 6px;
  background: var(--color-shadow);
  color: var(--color-text);
  border: 1px solid var(--color-border);
  padding: 0.25rem 0.5rem;
}
.mini-input-area button {
  border-radius: 6px;
  border: none;
  background: var(--color-theme);
  color: var(--color-dark);
  padding: 0.25rem 0.75rem;
  cursor: pointer;
}

/* Feedback buttons (identical to AiAssistant) */
.feedback-buttons {
  display: flex;
  gap: 0.5rem;
  margin-top: 0.5rem;
  opacity: 0.6;
  transition: opacity 0.2s;
}

.mini-message:hover .feedback-buttons {
  opacity: 1;
}

.feedback-btn {
  background: transparent;
  border: 1px solid rgba(0, 0, 0, 0.1);
  border-radius: 4px;
  padding: 0.25rem 0.5rem;
  cursor: pointer;
  display: flex;
  align-items: center;
  gap: 0.25rem;
  color: var(--color-text);
  transition: all 0.2s;
}

.feedback-btn:hover {
  background-color: rgba(0, 0, 0, 0.05);
  transform: scale(1.05);
}

.feedback-btn.thumbs-up:hover {
  color: green;
  border-color: green;
}

.feedback-btn.thumbs-down:hover {
  color: darkred;
  border-color: darkred;
}

.feedback-given {
  margin-top: 0.5rem;
  font-size: 0.85rem;
  opacity: 0.7;
  font-style: italic;
}
</style>