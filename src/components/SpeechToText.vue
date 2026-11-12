<template>
  <section class="speech-container" ref="containerEl">
    <div class="mic-group">
      <!-- Speech Button -->
      <button @click="toggleRecording" :class="['speech-btn', { recording: isRecording, disabled: !canRecord || isProcessing, processing: isProcessing }]" :disabled="!canRecord || isProcessing" :title="getButtonTitle()">
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

      <!-- Slim microphone picker toggle -->
      <button class="mic-picker-btn" :class="{ open: menuOpen, warn: permission !== 'granted' }" :disabled="isRecording" @click="toggleMenu" title="Choose microphone / allow access" aria-haspopup="menu" :aria-expanded="menuOpen">
        <svg xmlns="http://www.w3.org/2000/svg" width="10" height="10" viewBox="0 0 10 10" fill="var(--color-dark)"><path d="M5 2 9 8H1z"/></svg>
      </button>
    </div>

    <!-- Microphone menu (opens upwards) -->
    <div v-if="menuOpen" class="mic-menu" role="menu">
      <div class="mic-menu-header">
        <span>Microphone</span>
        <button class="mic-refresh" @click="refreshDevices" title="Refresh device list">⟳</button>
      </div>

      <button v-if="permission !== 'granted'" class="mic-allow" :disabled="busy || permission === 'unsupported'" @click="requestAccess">
        {{ busy ? 'Requesting…' : 'Allow microphone access' }}
      </button>

      <ul class="mic-list">
        <li>
          <button :class="{ selected: !selectedDeviceId }" @click="pick('')">
            <span class="check">{{ !selectedDeviceId ? '✓' : '' }}</span> System default
          </button>
        </li>
        <li v-for="(mic, i) in devices" :key="mic.deviceId">
          <button :class="{ selected: selectedDeviceId === mic.deviceId }" @click="pick(mic.deviceId)">
            <span class="check">{{ selectedDeviceId === mic.deviceId ? '✓' : '' }}</span> {{ micLabel(mic, i) }}
          </button>
        </li>
      </ul>
      <p v-if="permission === 'granted' && !devices.length" class="mic-hint">No microphones detected.</p>
      <p v-else-if="permission !== 'granted' && !devices.length" class="mic-hint">Allow access to see your microphones.</p>
      <p v-if="micError" class="mic-error">{{ micError }}</p>
    </div>
  </section>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useMicrophone, micLabel } from '../composables/useMicrophone'
import { WavRecorder } from '../utils/wavRecorder'

const emit = defineEmits<{
  transcription: [text: string]
  error: [error: string]
}>()

const {
  devices, selectedDeviceId, selectedLabel, permission, error: micError, busy,
  init, requestAccess, refreshDevices, selectDevice, openStream, isSupported,
} = useMicrophone()

const isRecording = ref(false)
const isProcessing = ref(false)
const menuOpen = ref(false)
const containerEl = ref<HTMLElement | null>(null)
const canRecord = computed(() => permission.value !== 'unsupported')

let mediaRecorder: MediaRecorder | null = null
let wavRecorder: WavRecorder | null = null
let audioChunks: Blob[] = []
let stream: MediaStream | null = null

// Prefer compressed WebM when the webview can record it; otherwise fall back to WAV
const pickRecorderMime = (): string | null => {
  if (typeof MediaRecorder === 'undefined') return null
  for (const type of ['audio/webm;codecs=opus', 'audio/webm']) {
    if (MediaRecorder.isTypeSupported(type)) return type
  }
  return null
}

const onDocPointerDown = (e: MouseEvent) => {
  if (menuOpen.value && containerEl.value && !containerEl.value.contains(e.target as Node)) {
    menuOpen.value = false
  }
}
const onKeyDown = (e: KeyboardEvent) => {
  if (e.key === 'Escape') menuOpen.value = false
}

onMounted(() => {
  init()
  document.addEventListener('mousedown', onDocPointerDown)
  document.addEventListener('keydown', onKeyDown)
})

onUnmounted(() => {
  document.removeEventListener('mousedown', onDocPointerDown)
  document.removeEventListener('keydown', onKeyDown)
  cleanupStream()
})

const toggleMenu = () => {
  menuOpen.value = !menuOpen.value
  if (menuOpen.value) refreshDevices()
}

const pick = (deviceId: string) => {
  selectDevice(deviceId)
  menuOpen.value = false
}

const getButtonTitle = () => {
  if (!isSupported()) return 'Microphone not available'
  if (isProcessing.value) return 'Processing audio...'
  if (isRecording.value) return 'Stop recording'
  if (permission.value !== 'granted') return 'Start voice input (will ask for microphone access)'
  return `Start voice input (${selectedLabel.value})`
}

const toggleRecording = async () => {
  if (isRecording.value) {
    await stopRecording()
  } else {
    await startRecording()
  }
}

const startRecording = async () => {
  if (permission.value !== 'granted') {
    const ok = await requestAccess()
    if (!ok) {
      menuOpen.value = true
      emit('error', micError.value || 'Microphone access is required for voice input.')
      return
    }
  }
  try {
    stream = await openStream({
      channelCount: 1,
      echoCancellation: false,
      noiseSuppression: false,
      autoGainControl: false,
    })

    const mimeType = pickRecorderMime()
    if (mimeType) {
      mediaRecorder = new MediaRecorder(stream, { mimeType })
      audioChunks = []
      mediaRecorder.ondataavailable = (event) => {
        if (event.data.size > 0) audioChunks.push(event.data)
      }
      mediaRecorder.onstop = async () => {
        const blob = new Blob(audioChunks, { type: mediaRecorder?.mimeType || mimeType })
        audioChunks = []
        cleanupStream()
        await process(new Uint8Array(await blob.arrayBuffer()), 'webm')
      }
      mediaRecorder.start(100)
    } else {
      wavRecorder = new WavRecorder(stream)
      await wavRecorder.start()
    }
    isRecording.value = true
  } catch (error) {
    console.error('Error starting recording:', error)
    cleanupStream()
    const name = (error as { name?: string })?.name
    if (name === 'NotAllowedError' || name === 'SecurityError') {
      permission.value = 'denied'
      menuOpen.value = true
      emit('error', 'Microphone access denied. Use the ▲ button next to the microphone to allow access.')
    } else if (name === 'NotFoundError') {
      emit('error', 'No microphone found. Please check your microphone connection.')
    } else if (name === 'NotReadableError') {
      emit('error', 'The microphone is busy or unavailable (another app may be using it).')
    } else {
      emit('error', `Failed to access microphone${name ? `: ${name}` : ''}`)
    }
  }
}

const stopRecording = async () => {
  if (!isRecording.value) return
  isRecording.value = false
  if (mediaRecorder) {
    mediaRecorder.stop() // onstop handles processing + cleanup
    mediaRecorder = null
  } else if (wavRecorder) {
    const rec = wavRecorder
    wavRecorder = null
    try {
      const wav = await rec.stop()
      cleanupStream()
      await process(wav, 'wav')
    } catch (e) {
      console.error('Error finishing WAV recording:', e)
      cleanupStream()
      emit('error', 'Failed to process recording')
    }
  }
}

const cleanupStream = () => {
  if (stream) {
    stream.getTracks().forEach(track => track.stop())
    stream = null
  }
}

const process = async (bytes: Uint8Array, format: 'webm' | 'wav') => {
  isProcessing.value = true
  try {
    await processRecording(bytes, format)
  } catch (error) {
    console.error('Error processing recording:', error)
    emit('error', 'Failed to process recording')
  } finally {
    isProcessing.value = false
  }
}

const processRecording = async (audio: Uint8Array, format: string) => {
  // ~0.1 s of audio at minimum (WAV header alone is 44 bytes)
  if (audio.length <= 44) {
    emit('error', 'No audio data recorded')
    return
  }

  try {
    const result = await invoke<string>('transcribe_audio', {
      audioData: Array.from(audio),
      format
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

.speech-container {
  position: relative;
}

/* mic + arrow render as one split button: only the outer corners are rounded */
.mic-group {
  display: flex;
  align-items: stretch;
  gap: 0;
  border-radius: 12px;
  transition: box-shadow 0.15s ease;
}

.mic-group:hover {
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.2);
}

.mic-picker-btn {
  width: 14px;
  height: 40px;
  padding: 0;
  border: none;
  /* thin divider between the two halves */
  border-left: 1px solid rgba(0, 0, 0, 0.25);
  border-radius: 0 12px 12px 0;
  background-color: var(--color-theme);
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: filter 0.15s ease;
}

.mic-picker-btn:hover:not(:disabled),
.mic-picker-btn.open {
  filter: brightness(1.12);
}

.mic-picker-btn.open svg {
  transform: rotate(180deg);
}

.mic-picker-btn.warn {
  box-shadow: inset 0 -3px 0 var(--color-error, #e74c3c);
}

.mic-picker-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.mic-menu {
  position: absolute;
  bottom: calc(100% + 0.5rem);
  right: 0;
  z-index: 50;
  width: max-content;
  min-width: 14rem;
  max-width: min(22rem, 80vw);
  padding: 0.5rem;
  border-radius: 8px;
  border: 1px solid var(--color-theme);
  background: var(--color-main);
  color: var(--color-text);
  box-shadow: 0 6px 20px rgba(0, 0, 0, 0.25);
  display: flex;
  flex-direction: column;
  gap: 0.4rem;
  font-size: 0.85rem;
}

.mic-menu-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  font-weight: bold;
  color: var(--color-theme);
}

.mic-refresh {
  background: none;
  border: none;
  color: var(--color-text);
  cursor: pointer;
  font-size: 1rem;
}

.mic-allow {
  padding: 0.4rem 0.6rem;
  border: none;
  border-radius: 6px;
  background: var(--color-theme);
  color: var(--color-dark);
  cursor: pointer;
}

.mic-allow:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.mic-list {
  list-style: none;
  margin: 0;
  padding: 0;
  max-height: 14rem;
  overflow-y: auto;
}

.mic-list button {
  width: 100%;
  text-align: left;
  padding: 0.35rem 0.4rem;
  border: none;
  border-radius: 4px;
  background: none;
  color: var(--color-text);
  cursor: pointer;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.mic-list button:hover {
  background: var(--color-shadow);
}

.mic-list button.selected {
  color: var(--color-theme);
}

.check {
  display: inline-block;
  width: 1rem;
}

.mic-hint {
  opacity: 0.7;
  margin: 0;
}

.mic-error {
  color: var(--color-error, #e74c3c);
  margin: 0;
}

.speech-btn {
  padding: 0.5rem;
  border-radius: 12px 0 0 12px;
  border: none;
  cursor: pointer;
  background-color: var(--color-theme);
  color: var(--color-dark);
  display: flex;
  align-items: center;
  justify-content: center;
  transition: filter 0.15s ease;
  width: 40px;
  height: 40px;
  position: relative;
}

/* no scaling on hover: it would break the joined shape */
.speech-btn:hover:not(.disabled) {
  filter: brightness(1.12);
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
    filter: brightness(1);
  }
  50% {
    filter: brightness(1.3);
  }
  100% {
    filter: brightness(1);
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