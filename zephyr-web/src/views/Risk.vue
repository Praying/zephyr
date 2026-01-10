<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { ElMessage } from 'element-plus'
import type { RiskLimit } from '@/types'

const riskLimits = ref<RiskLimit[]>([])
const showEditDialog = ref(false)
const editingLimit = ref<RiskLimit | null>(null)

function getStatusType(status: string) {
  const map: Record<string, string> = { normal: 'success', warning: 'warning', breached: 'danger' }
  return map[status] || 'info'
}

function getUsagePercent(limit: RiskLimit): number {
  return Math.min((limit.current / limit.limit) * 100, 100)
}

function getProgressStatus(limit: RiskLimit) {
  const percent = getUsagePercent(limit)
  if (percent >= 100) return 'exception'
  if (percent >= 80) return 'warning'
  return ''
}

function editLimit(limit: RiskLimit) {
  editingLimit.value = { ...limit }
  showEditDialog.value = true
}

function saveLimit() {
  if (!editingLimit.value) return
  const index = riskLimits.value.findIndex(
    (l) => l.type === editingLimit.value!.type && l.symbol === editingLimit.value!.symbol
  )
  if (index >= 0) {
    riskLimits.value[index] = { ...editingLimit.value }
  }
  showEditDialog.value = false
  ElMessage.success('Risk limit updated')
}

function formatLimitType(type: string): string {
  const map: Record<string, string> = {
    position: 'Position Limit',
    order_rate: 'Order Rate Limit',
    daily_loss: 'Daily Loss Limit',
  }
  return map[type] || type
}

onMounted(() => {
  riskLimits.value = [
    { type: 'position', symbol: 'BTCUSDT', limit: 100000, current: 45000, status: 'normal' },
    { type: 'position', symbol: 'ETHUSDT', limit: 50000, current: 42000, status: 'warning' },
    { type: 'order_rate', limit: 100, current: 35, status: 'normal' },
    { type: 'daily_loss', limit: 5000, current: 1200, status: 'normal' },
  ]
})
</script>

<template>
  <div class="risk-page">
    <h1 class="page-title">Risk Management</h1>
    <div class="risk-grid">
      <div v-for="limit in riskLimits" :key="`${limit.type}-${limit.symbol}`" class="card risk-card">
        <div class="risk-header">
          <div>
            <div class="risk-type">{{ formatLimitType(limit.type) }}</div>
            <div v-if="limit.symbol" class="risk-symbol">{{ limit.symbol }}</div>
          </div>
          <el-tag :type="getStatusType(limit.status)" size="small">{{ limit.status }}</el-tag>
        </div>
        <div class="risk-values">
          <span class="current">{{ limit.current.toLocaleString() }}</span>
          <span class="separator">/</span>
          <span class="limit">{{ limit.limit.toLocaleString() }}</span>
        </div>
        <el-progress
          :percentage="getUsagePercent(limit)"
          :status="getProgressStatus(limit)"
          :stroke-width="8"
          :show-text="false"
        />
        <div class="risk-actions">
          <el-button size="small" @click="editLimit(limit)">Edit Limit</el-button>
        </div>
      </div>
    </div>

    <el-dialog v-model="showEditDialog" title="Edit Risk Limit" width="400px">
      <el-form v-if="editingLimit" label-position="top">
        <el-form-item label="Type">
          <el-input :model-value="formatLimitType(editingLimit.type)" disabled />
        </el-form-item>
        <el-form-item v-if="editingLimit.symbol" label="Symbol">
          <el-input :model-value="editingLimit.symbol" disabled />
        </el-form-item>
        <el-form-item label="Limit">
          <el-input-number v-model="editingLimit.limit" :min="0" style="width: 100%" />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="showEditDialog = false">Cancel</el-button>
        <el-button type="primary" @click="saveLimit">Save</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<style scoped lang="scss">
.risk-page { padding: 20px; }
.page-title { font-size: 24px; font-weight: 600; margin-bottom: 20px; }
.risk-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(280px, 1fr)); gap: 16px; }
.risk-card { display: flex; flex-direction: column; gap: 12px; }
.risk-header { display: flex; justify-content: space-between; align-items: flex-start; }
.risk-type { font-weight: 600; }
.risk-symbol { font-size: 12px; color: var(--z-text-secondary); }
.risk-values { font-size: 20px; .current { font-weight: 600; } .separator, .limit { color: var(--z-text-secondary); } }
.risk-actions { margin-top: auto; }
</style>
