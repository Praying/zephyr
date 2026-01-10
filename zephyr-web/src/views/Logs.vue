<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import type { LogEntry } from '@/types'

const logs = ref<LogEntry[]>([])
const filterLevel = ref('')
const filterComponent = ref('')
const searchText = ref('')
const autoScroll = ref(true)
const logContainer = ref<HTMLElement | null>(null)

const filteredLogs = computed(() => {
  return logs.value.filter((log) => {
    if (filterLevel.value && log.level !== filterLevel.value) return false
    if (filterComponent.value && !log.component.includes(filterComponent.value)) return false
    if (searchText.value && !log.message.toLowerCase().includes(searchText.value.toLowerCase())) return false
    return true
  })
})

const components = computed(() => [...new Set(logs.value.map((l) => l.component))])

function getLevelType(level: string) {
  const map: Record<string, string> = { debug: 'info', info: '', warn: 'warning', error: 'danger' }
  return map[level] || ''
}

function formatTime(ts: number): string {
  return new Date(ts).toLocaleTimeString('en-US', { hour12: false, fractionalSecondDigits: 3 })
}

function clearLogs() {
  logs.value = []
}

function addMockLog() {
  const levels: LogEntry['level'][] = ['debug', 'info', 'warn', 'error']
  const comps = ['engine', 'gateway.binance', 'strategy.btc_trend', 'risk', 'api']
  const messages = [
    'Tick received for BTCUSDT',
    'Order submitted successfully',
    'Position updated',
    'WebSocket reconnecting...',
    'Rate limit warning',
    'Strategy signal generated',
  ]
  logs.value.push({
    timestamp: Date.now(),
    level: levels[Math.floor(Math.random() * levels.length)],
    component: comps[Math.floor(Math.random() * comps.length)],
    message: messages[Math.floor(Math.random() * messages.length)],
  })
  if (logs.value.length > 500) logs.value.shift()
  if (autoScroll.value && logContainer.value) {
    logContainer.value.scrollTop = logContainer.value.scrollHeight
  }
}

let interval: ReturnType<typeof setInterval>
onMounted(() => {
  for (let i = 0; i < 20; i++) addMockLog()
  interval = setInterval(addMockLog, 2000)
})
onUnmounted(() => clearInterval(interval))
</script>

<template>
  <div class="logs-page">
    <div class="page-header">
      <h1 class="page-title">Logs</h1>
      <div class="header-actions">
        <el-input v-model="searchText" placeholder="Search..." clearable style="width: 200px" />
        <el-select v-model="filterLevel" placeholder="Level" clearable style="width: 120px">
          <el-option label="Debug" value="debug" />
          <el-option label="Info" value="info" />
          <el-option label="Warn" value="warn" />
          <el-option label="Error" value="error" />
        </el-select>
        <el-select v-model="filterComponent" placeholder="Component" clearable style="width: 160px">
          <el-option v-for="c in components" :key="c" :label="c" :value="c" />
        </el-select>
        <el-checkbox v-model="autoScroll">Auto-scroll</el-checkbox>
        <el-button @click="clearLogs">Clear</el-button>
      </div>
    </div>
    <div class="card log-container" ref="logContainer">
      <div v-for="(log, index) in filteredLogs" :key="index" class="log-entry" :class="log.level">
        <span class="log-time">{{ formatTime(log.timestamp) }}</span>
        <el-tag :type="getLevelType(log.level)" size="small" class="log-level">{{ log.level.toUpperCase() }}</el-tag>
        <span class="log-component">[{{ log.component }}]</span>
        <span class="log-message">{{ log.message }}</span>
      </div>
    </div>
  </div>
</template>

<style scoped lang="scss">
.logs-page { padding: 20px; display: flex; flex-direction: column; height: calc(100vh - 100px); }
.page-header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 16px; flex-shrink: 0; }
.page-title { font-size: 24px; font-weight: 600; margin: 0; }
.header-actions { display: flex; gap: 12px; align-items: center; }
.log-container { flex: 1; overflow-y: auto; font-family: 'Fira Code', monospace; font-size: 13px; padding: 12px; }
.log-entry {
  display: flex; gap: 8px; padding: 4px 0; border-bottom: 1px solid var(--z-border-color);
  &.error { background-color: rgba(239, 68, 68, 0.1); }
  &.warn { background-color: rgba(245, 158, 11, 0.1); }
}
.log-time { color: var(--z-text-secondary); min-width: 100px; }
.log-level { min-width: 60px; text-align: center; }
.log-component { color: var(--el-color-primary); min-width: 160px; }
.log-message { flex: 1; }
</style>
