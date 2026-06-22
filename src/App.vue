<template>
  <div>
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
      </main>
    </div>
  </div>
</template>

<script setup>
import { ref, onMounted } from 'vue'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { invoke } from '@tauri-apps/api/core'
import ActivationScreen from './components/ActivationScreen.vue'
import ClientSelector from './components/ClientSelector.vue'
import FileSelector from './components/FileSelector.vue'
import UploadHistorySection from './components/UploadHistorySection.vue'
import NetworkStatus from './components/NetworkStatus.vue'

const isActivated = ref(!!localStorage.getItem('fluxbooks_session'))
const uploads = ref([])

// Load all uploads from SQLite on start
onMounted(async () => {
  if (isActivated.value) {
    await loadQueue()
    // Poll every 3 seconds to refresh progress
    setInterval(loadQueue, 3000)
  }
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

// New file queued — reload from SQLite
async function onUploadQueued() {
  await loadQueue()
}

// Internet came back — tell Rust to retry all pending/failed
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
</style>