<template>
  <div class="card">
    <button class="select-btn" @click="handleSelectFile" :disabled="isQueuing">
      {{ isQueuing ? 'Adding to queue…' : 'Select File' }}
    </button>

    <p v-if="message" class="message" :class="messageType">{{ message }}</p>
  </div>
</template>

<script setup>
import { ref } from 'vue'
import { open } from '@tauri-apps/plugin-dialog'
import { invoke } from '@tauri-apps/api/core'

const emit = defineEmits(['upload-queued'])

const isQueuing = ref(false)
const message   = ref('')
const messageType = ref('')

async function handleSelectFile() {
  const filePath = await open({
    multiple: false,
    directory: false,
  })

  if (!filePath) return

  isQueuing.value = true
  message.value = ''

  try {
    // Call Rust: copy file to secure folder + save to SQLite
    const record = await invoke('queue_file', {
      filePath,
      isOnline: navigator.onLine,
    })

    // Tell App.vue about the new queued file
    emit('upload-queued', record)

    message.value = navigator.onLine
      ? '✅ File queued — uploading now'
      : '📦 Saved locally — will upload when online'
    messageType.value = navigator.onLine ? 'success' : 'warning'

  } catch (err) {
    message.value = `❌ Error: ${err}`
    messageType.value = 'error'
  } finally {
    isQueuing.value = false
    setTimeout(() => message.value = '', 4000)
  }
}
</script>

<style scoped>
.select-btn {
  width: 100%;
  padding: 10px 16px;
  font-size: 14px;
  background-color: #2d6cdf;
  color: white;
  border: none;
  border-radius: 6px;
  cursor: pointer;
  transition: background-color 0.15s;
}

.select-btn:hover:not(:disabled) { background-color: #1f54b3; }
.select-btn:disabled { background-color: #a0b4e0; cursor: default; }

.message {
  margin-top: 10px;
  font-size: 13px;
  text-align: center;
}
.success { color: #27ae60; }
.warning { color: #e67e22; }
.error   { color: #e74c3c; }
</style>