<template>
  <div class="network-status" :class="isOnline ? 'status-online' : 'status-offline'">
    <span class="status-dot">{{ isOnline ? '🟢' : '🔴' }}</span>
    <span class="status-text">{{ isOnline ? 'Online' : 'Offline' }}</span>
  </div>
</template>

<script setup>
import { ref, onMounted, onUnmounted } from 'vue'

const emit = defineEmits(['online', 'offline'])
const isOnline = ref(navigator.onLine)

function handleOnline() {
  isOnline.value = true
  emit('online')   // ← tells App.vue to retry pending uploads
}

function handleOffline() {
  isOnline.value = false
  emit('offline')
}

onMounted(() => {
  window.addEventListener('online', handleOnline)
  window.addEventListener('offline', handleOffline)
})

onUnmounted(() => {
  window.removeEventListener('online', handleOnline)
  window.removeEventListener('offline', handleOffline)
})
</script>

<style scoped>
.network-status {
  display: flex; align-items: center; gap: 8px;
  padding: 8px 14px; border-radius: 20px;
  font-size: 13px; font-weight: bold;
  width: fit-content; margin: 0 auto 8px auto;
  transition: background-color 0.3s ease;
}
.status-online  { background-color: #eafaf1; color: #1e8449; border: 1px solid #a9dfbf; }
.status-offline { background-color: #fdedec; color: #c0392b; border: 1px solid #f1948a; }
.status-dot { font-size: 12px; flex-shrink: 0; }
</style>

<style scoped>
.network-status {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 14px;
  border-radius: 20px;
  font-size: 13px;
  font-weight: bold;
  /* Center the pill horizontally under the heading */
  width: fit-content;
  margin: 0 auto 8px auto;
  /* Smooth background color transition when status changes */
  transition: background-color 0.3s ease;
}

.status-online {
  background-color: #eafaf1;
  color: #1e8449;
  border: 1px solid #a9dfbf;
}

.status-offline {
  background-color: #fdedec;
  color: #c0392b;
  border: 1px solid #f1948a;
}

.status-dot {
  font-size: 12px;
  /* Prevent the emoji from being squished on narrow windows */
  flex-shrink: 0;
}

.status-text {
  letter-spacing: 0.3px;
}
</style>