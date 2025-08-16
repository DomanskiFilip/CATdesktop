<template>
  <section class="speech-container">
    <!-- Speech Button -->
    <button @click="toggleRecording" :class="['speech-btn', { recording: isRecording, disabled: !isSupported || isProcessing, processing: isProcessing }]" :disabled="!isSupported || isProcessing" :title="getButtonTitle()">
      <!-- Microphone Icon (not recording, not processing) -->
      <svg v-if="!isRecording && !isProcessing" xmlns="http://www.w3.org/2000/svg" height="20px" viewBox="0 -960 960 960" width="20px" fill="var(--color-dark)"><path d="M480-400q-50 0-85-35t-35-85v-240q0-50 35-85t85-35q50 0 85 35t35 85v240q0 50-35 85t-85 35Zm0-240Zm-40 520v-123q-104-14-172-93t-68-184h80q0 83 58.5 141.5T480-320q83 0 141.5-58.5T680-520h80q0 105-68 184t-172 93v123h-40Zm40-360q17 0 28.5-11.5T520-520v-240q0-17-11.5-28.5T480-800q-17 0-28.5 11.5T440-760v240q0 17 11.5 28.5T480-480Z"/></svg>
      <!-- Stop Icon (recording) -->
      <svg v-else-if="isRecording" xmlns="http://www.w3.org/2000/svg" height="20px" viewBox="0 -960 960 960" width="20px" fill="var(--color-dark)"><path d="M320-320h320v-320H320v320ZM480-80q-83 0-156-31.5T197-197q-54-54-85.5-127T80-480q0-83 31.5-156T197-763q54-54 127-85.5T480-880q83 0 156 31.5T763-763q54 54 85.5 127T880-480q0 83-31.5 156T763-197q-54 54-127 85.5T480-80Z"/></svg>
      <!-- Loading Spinner (processing) -->
      <div v-else-if="isProcessing" class="loading-spinner">
        <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="var(--color-dark)" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M21 12a9 9 0 11-6.219-8.56"/>
        </svg>
      </div>
    </button>

    <!-- Microphone Selection Dropdown -->
    <select v-if="!isRecording && availableMicrophones.length > 1" v-model="selectedMicrophoneId" @change="onMicrophoneChange" class="microphone-select" title="Select microphone">
      <option v-for="mic in availableMicrophones" :key="mic.deviceId" :value="mic.deviceId">
        {{ mic.label || `Microphone ${mic.deviceId.slice(0, 8)}` }}
      </option>
    </select>
  </section>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'

const emit = defineEmits<{
  transcription: [text: string]
  error: [error: string]
}>()

const isRecording = ref(false)
const isSupported = ref(false)
const isProcessing = ref(false)
const availableMicrophones = ref<MediaDeviceInfo[]>([])
const selectedMicrophoneId = ref<string>('')
const selectedMicrophoneLabel = ref<string>('')

let mediaRecorder: MediaRecorder | null = null
let audioChunks: Blob[] = []
let stream: MediaStream | null = null
let audioContext: AudioContext | null = null
let animationFrame: number | null = null

onMounted(async () => {
  isSupported.value = !!(navigator.mediaDevices && navigator.mediaDevices.getUserMedia)
  
  if (isSupported.value) {
    await loadAvailableMicrophones()
  }
})

onUnmounted(() => {
  stopRecording()
  if (audioContext) {
    audioContext.close()
  }
  if (animationFrame) {
    cancelAnimationFrame(animationFrame)
  }
})

const loadAvailableMicrophones = async () => {
  try {
    const tempStream = await navigator.mediaDevices.getUserMedia({ audio: true })
    tempStream.getTracks().forEach(track => track.stop())
    
    const devices = await navigator.mediaDevices.enumerateDevices()
    availableMicrophones.value = devices.filter(device => device.kind === 'audioinput')
    
    if (availableMicrophones.value.length > 0) {
      selectedMicrophoneId.value = availableMicrophones.value[0].deviceId
      selectedMicrophoneLabel.value = availableMicrophones.value[0].label || 'Default Microphone'
    }
  } catch (error) {
    console.error('Error loading microphones:', error)
    emit('error', 'Failed to access microphones')
  }
}

const onMicrophoneChange = () => {
  const selectedMic = availableMicrophones.value.find(mic => mic.deviceId === selectedMicrophoneId.value)
  selectedMicrophoneLabel.value = selectedMic?.label || 'Unknown Microphone'
}

const getButtonTitle = () => {
  if (!isSupported.value) return 'Speech recognition not supported'
  if (isProcessing.value) return 'Processing audio...'
  if (isRecording.value) return 'Stop recording'
  return `Start voice input (${selectedMicrophoneLabel.value})`
}

const toggleRecording = async () => {
  if (isRecording.value) {
    stopRecording()
  } else {
    await startRecording()
  }
}

const startRecording = async () => {
  try {
    const constraints = {
      audio: {
        deviceId: selectedMicrophoneId.value ? { exact: selectedMicrophoneId.value } : undefined,
        sampleRate: 16000,
        channelCount: 1,
        echoCancellation: false,
        noiseSuppression: false,
        autoGainControl: false,
        volume: 1.0
      }
    }
    
    stream = await navigator.mediaDevices.getUserMedia(constraints)
    
    // Set up audio level monitoring with proper Web Audio API usage
    audioContext = new AudioContext()
    
    // Resume audio context if it's suspended (required by some browsers)
    if (audioContext.state === 'suspended') {
      await audioContext.resume()
    }
    
    // Create MediaRecorder
    const options = { mimeType: 'audio/webm;codecs=opus' }
    
    if (!MediaRecorder.isTypeSupported(options.mimeType)) {
      options.mimeType = 'audio/webm'
      if (!MediaRecorder.isTypeSupported(options.mimeType)) {
        options.mimeType = 'audio/wav'
      }
    }
    
    mediaRecorder = new MediaRecorder(stream, options)
    audioChunks = []
    
    mediaRecorder.ondataavailable = (event) => {
      if (event.data.size > 0) {
        audioChunks.push(event.data)
      }
    }
    
    mediaRecorder.onstop = async () => {
      try {
        isProcessing.value = true
        await processRecording()
      } catch (error) {
        console.error('Error processing recording:', error)
        emit('error', 'Failed to process recording')
      } finally {
        isProcessing.value = false
      }
    }
    
    mediaRecorder.start(100)
    isRecording.value = true
    
  } catch (error) {
    console.error('Error starting recording:', error)
    if (typeof error === 'object' && error !== null && 'name' in error) {
      const errorName = (error as { name?: string }).name
      if (errorName === 'NotAllowedError') {
        emit('error', 'Microphone access denied. Please allow microphone access and try again.')
      } else if (errorName === 'NotFoundError') {
        emit('error', 'No microphone found. Please check your microphone connection.')
      } else if (errorName === 'OverconstrainedError') {
        emit('error', 'Selected microphone is not available. Please try a different microphone.')
      } else {
        emit('error', `Microphone error: ${errorName}`)
      }
    } else {
      emit('error', 'Failed to access microphone')
    }
  }
}

const stopRecording = () => {
  if (animationFrame) {
    cancelAnimationFrame(animationFrame)
    animationFrame = null
  }
  
  if (mediaRecorder && isRecording.value) {
    mediaRecorder.stop()
    isRecording.value = false
  }
  
  if (stream) {
    stream.getTracks().forEach(track => track.stop())
    stream = null
  }
  
  if (audioContext) {
    audioContext.close()
    audioContext = null
  }
}

const processRecording = async () => {
  if (audioChunks.length === 0) {
    emit('error', 'No audio data recorded')
    return
  }
  
  try {
    const audioBlob = new Blob(audioChunks, { type: mediaRecorder?.mimeType || 'audio/webm' })
    const arrayBuffer = await audioBlob.arrayBuffer()
    const uint8Array = new Uint8Array(arrayBuffer)
    
    const mimeType = mediaRecorder?.mimeType || 'audio/webm'
    const format = mimeType.includes('webm') ? 'webm' : 
                  mimeType.includes('wav') ? 'wav' : 
                  mimeType.includes('mp3') ? 'mp3' : 'webm'
    
    const result = await invoke<string>('transcribe_audio', { 
      audioData: Array.from(uint8Array),
      format: format
    })
    
    if (typeof result === 'string' && (result.startsWith('RATE_LIMIT:') || result.startsWith('Session expired') || result.startsWith('Speech recognition error:'))) {
      emit('error', result)
      return
    }

    let transcriptionResponse
    try {
      transcriptionResponse = JSON.parse(result)
    } catch (parseError) {
      emit('error', result)
      return
    }

    if (transcriptionResponse.transcription && transcriptionResponse.transcription.trim()) {
      emit('transcription', transcriptionResponse.transcription.trim())
    } else {
      emit('error', 'No speech detected. Please try speaking more clearly.')
    }

  } catch (error) {
    console.error('Error processing audio:', error)
    const errorMessage = typeof error === 'string' ? error : (error instanceof Error ? error.message : JSON.stringify(error))
    
    if (errorMessage.includes('RATE_LIMIT:')) {
      emit('error', errorMessage)
    } else if (errorMessage.includes('🚫')) {
      emit('error', errorMessage)
    } else if (errorMessage.includes('not logged in')) {
      emit('error', 'Please log in to use speech-to-text')
    } else if (errorMessage.includes('Session expired')) {
      emit('error', 'Session expired. Please log in again.')
    } else {
      emit('error', 'Failed to transcribe audio. Please try again.')
    }
  }
}
</script>

<style scoped>
.speech-container {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 0.5rem;
}

.microphone-select {
  padding: 0.25rem 0.5rem;
  border: 1px solid var(--color-border);
  border-radius: 4px;
  background-color: var(--color-main);
  color: var(--color-text);
  font-size: 0.8rem;
  max-width: 200px;
}

.microphone-select option {
  cursor: pointer;
  background-color: var(--color-main);
  color: var(--color-text);
}

.microphone-select option:hover, 
.microphone-select option:focus {
  background-color: var(--color-text);
  color: var(--color-dark);
  cursor: pointer;
}

.speech-btn {
  padding: 0.5rem;
  border-radius: 30%;
  border: none;
  cursor: pointer;
  background-color: var(--color-theme);
  color: var(--color-dark);
  display: flex;
  align-items: center;
  justify-content: center;
  transition: transform 0.1s ease;
  width: 40px;
  height: 40px;
  position: relative;
}

.speech-btn:hover:not(.disabled) {
  transform: scale(1.05);
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.2);
}

.speech-btn.recording {
  background-color: #e74c3c;
  animation: pulse 1s infinite;
}

.speech-btn.disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.loading-spinner {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 20px;
  height: 20px;
}

.loading-spinner svg {
  animation: spin 1s linear infinite;
}

@keyframes pulse {
  0% {
    transform: scale(1);
  }
  50% {
    transform: scale(1.05);
  }
  100% {
    transform: scale(1);
  }
}

@keyframes spin {
  from {
    transform: rotate(0deg);
  }
  to {
    transform: rotate(360deg);
  }
}
</style>