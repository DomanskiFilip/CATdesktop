<template>
  <div class="mini-ai-chat" v-if="!props.disabled">
    <div class="mini-message" v-if="clarifyingQuestion">
      <span class="ai-label">CAT:</span> {{ clarifyingQuestion }}
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

const props = defineProps<{
  clarifyingQuestion: string
  disabled?: boolean
}>()
const emit = defineEmits(['answered'])

const userInput = ref('')
const userAnswer = ref('')
const isProcessing = ref(false)

const sendAnswer = () => {
  if (!userInput.value.trim()) return
  userAnswer.value = userInput.value
  isProcessing.value = true
  emit('answered', userInput.value)
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
</style>