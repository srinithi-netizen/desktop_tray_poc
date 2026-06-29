<template>
  <!-- Startup loading screen -->
  <div v-if="startupError" class="error-screen">
    <h2>❌ Startup Failed</h2>
    <p>{{ startupError }}</p>
  </div>

  <div v-else-if="!ready" class="loading-screen">
    <div class="spinner"></div>
    <p>{{ startupMessage }}</p>
  </div>

  <!-- Your existing app -->
  <div v-else>
    <ActivationScreen v-if="!isActivated" @activated="onActivated" />

    <div v-else class="app-container">
      <header class="app-header">
        <h1>FluxBooks Desktop Tray</h1>
        <NetworkStatus @online="onOnline" @offline="onOffline" />
        <button class="tray-btn" @click="minimizeToTray">Minimize to Tray</button>
      </header>

      <main class="app-main">
        <ClientSelector />
        <FileSelector @upload-queued="onUploadQueued" />
        <UploadHistorySection
          :uploads="uploads"
          @upload-deleted="onUploadDeleted"
        />
        <BankDashboard />
      </main>
    </div>
  </div>
</template>

<script setup>
import { ref, onMounted } from 'vue'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import ActivationScreen from './components/ActivationScreen.vue'
import ClientSelector from './components/ClientSelector.vue'
import FileSelector from './components/FileSelector.vue'
import UploadHistorySection from './components/UploadHistorySection.vue'
import NetworkStatus from './components/NetworkStatus.vue'
import BankDashboard from './components/BankDashboard.vue'

// Startup state
const ready = ref(false)
const startupMessage = ref('Starting...')
const startupError = ref('')

// App state
const isActivated = ref(!!localStorage.getItem('fluxbooks_session'))
const uploads = ref([])

onMounted(async () => {
  // Listen for startup progress from Rust orchestrator
  await listen('startup_progress', (event) => {
    startupMessage.value = event.payload.message

    if (event.payload.step === 'ready') {
      ready.value = true
      if (isActivated.value) {
        loadQueue()
        setInterval(loadQueue, 3000)
      }
    }

    if (event.payload.step.startsWith('error')) {
      startupError.value = event.payload.message
    }
  })
})

async function loadQueue() {
  try {
    uploads.value = await invoke('get_queue')
  } catch (e) {
    console.error('Failed to load queue:', e)
  }
}

function onActivated() {
  isActivated.value = true
  loadQueue()
  setInterval(loadQueue, 3000)
}

async function onUploadQueued() {
  await loadQueue()
}

async function onOnline() {
  try {
    await invoke('retry_pending')
  } catch (e) {
    console.error('Retry failed:', e)
  }
}

function onOffline() {
  console.log('Gone offline — uploads will resume when connected')
}

async function onUploadDeleted(id) {
  try {
    await invoke('delete_upload', { id })
    await loadQueue()
  } catch (e) {
    console.error('Delete failed:', e)
  }
}

async function minimizeToTray() {
  await getCurrentWindow().hide()
}
</script>

<style>
* { box-sizing: border-box; }
body { margin: 0; font-family: Arial, sans-serif; background-color: #f4f5f7; }
.app-container { max-width: 480px; margin: 0 auto; padding: 24px; }
.app-header {
  display: flex; flex-direction: column;
  align-items: center; gap: 10px; margin-bottom: 24px;
}
.app-header h1 { font-size: 20px; color: #2d2d2d; margin: 0; }
.tray-btn {
  padding: 6px 14px; font-size: 12px;
  background-color: #f0f0f0; color: #444;
  border: 1px solid #ccc; border-radius: 4px; cursor: pointer;
}
.tray-btn:hover { background-color: #e0e0e0; }
.app-main { display: flex; flex-direction: column; gap: 20px; }
.card {
  background: white; border-radius: 8px;
  padding: 16px; box-shadow: 0 1px 3px rgba(0,0,0,0.1);
}

/* Startup screens */
.loading-screen, .error-screen {
  display: flex; flex-direction: column;
  align-items: center; justify-content: center;
  height: 100vh; gap: 16px; font-family: sans-serif;
}
.spinner {
  width: 40px; height: 40px;
  border: 4px solid #e2e8f0;
  border-top-color: #2563eb;
  border-radius: 50%;
  animation: spin 0.8s linear infinite;
}
@keyframes spin { to { transform: rotate(360deg); } }
.error-screen h2 { color: #ef4444; }
</style>