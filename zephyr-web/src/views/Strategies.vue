<script setup lang="ts">
import { ref, computed } from 'vue'
import { useTradingStore } from '@/stores/trading'
import { ElMessage, ElMessageBox } from 'element-plus'
import { VideoPlay, VideoPause } from '@element-plus/icons-vue'
import StrategyPerformanceChart from '@/components/charts/StrategyPerformanceChart.vue'
import type { Strategy } from '@/types'

const tradingStore = useTradingStore()
const selectedStrategy = ref<Strategy | null>(null)

const strategies = computed(() => tradingStore.strategies)

function getStatusType(status: string) {
  const map: Record<string, string> = {
    running: 'success',
    stopped: 'info',
    paused: 'warning',
    error: 'danger',
  }
  return map[status] || 'info'
}

function formatPnl(value: number): string {
  const prefix = value >= 0 ? '+' : ''
  return `${prefix}$${value.toLocaleString()}`
}

function formatPercent(value: number): string {
  return `${(value * 100).toFixed(1)}%`
}

async function toggleStrategy(strategy: Strategy) {
  const action = strategy.status === 'running' ? 'stop' : 'start'
  try {
    await ElMessageBox.confirm(
      `Are you sure you want to ${action} strategy "${strategy.name}"?`,
      'Confirm',
      { confirmButtonText: 'Yes', cancelButtonText: 'Cancel', type: 'warning' }
    )
    // Mock API call
    const newStatus = action === 'start' ? 'running' : 'stopped'
    const updated = strategies.value.map((s) =>
      s.id === strategy.id ? { ...s, status: newStatus as Strategy['status'] } : s
    )
    tradingStore.updateStrategies(updated)
    ElMessage.success(`Strategy ${action}ed successfully`)
  } catch {
    // User cancelled
  }
}

function selectStrategy(strategy: Strategy) {
  selectedStrategy.value = strategy
}
</script>

<template>
  <div class="strategies-page">
    <h1 class="page-title">Strategies</h1>
    <div class="content-grid">
      <div class="card strategies-list">
        <el-table :data="strategies" @row-click="selectStrategy" highlight-current-row>
          <el-table-column prop="name" label="Name" min-width="120" />
          <el-table-column prop="type" label="Type" width="80">
            <template #default="{ row }">
              <el-tag size="small">{{ row.type.toUpperCase() }}</el-tag>
            </template>
          </el-table-column>
          <el-table-column prop="status" label="Status" width="100">
            <template #default="{ row }">
              <el-tag :type="getStatusType(row.status)" size="small">{{ row.status }}</el-tag>
            </template>
          </el-table-column>
          <el-table-column prop="pnl" label="PnL" width="120">
            <template #default="{ row }">
              <span :class="row.pnl >= 0 ? 'profit' : 'loss'">{{ formatPnl(row.pnl) }}</span>
            </template>
          </el-table-column>
          <el-table-column prop="winRate" label="Win Rate" width="100">
            <template #default="{ row }">{{ formatPercent(row.winRate) }}</template>
          </el-table-column>
          <el-table-column label="Actions" width="100" fixed="right">
            <template #default="{ row }">
              <el-button
                :icon="row.status === 'running' ? VideoPause : VideoPlay"
                :type="row.status === 'running' ? 'warning' : 'success'"
                circle
                size="small"
                @click.stop="toggleStrategy(row)"
              />
            </template>
          </el-table-column>
        </el-table>
      </div>
      <div class="card strategy-detail" v-if="selectedStrategy">
        <h3>{{ selectedStrategy.name }} Performance</h3>
        <div class="detail-stats">
          <div class="stat"><span class="label">Trades</span><span class="value">{{ selectedStrategy.trades }}</span></div>
          <div class="stat"><span class="label">Sharpe</span><span class="value">{{ selectedStrategy.sharpeRatio.toFixed(2) }}</span></div>
          <div class="stat"><span class="label">Max DD</span><span class="value">{{ formatPercent(selectedStrategy.maxDrawdown) }}</span></div>
        </div>
        <StrategyPerformanceChart :strategy-id="selectedStrategy.id" />
      </div>
    </div>
  </div>
</template>

<style scoped lang="scss">
.strategies-page { padding: 20px; }
.page-title { font-size: 24px; font-weight: 600; margin-bottom: 20px; }
.content-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 16px; }
.strategies-list { overflow: hidden; }
.strategy-detail {
  h3 { margin-bottom: 16px; font-size: 16px; font-weight: 600; }
}
.detail-stats {
  display: flex;
  gap: 24px;
  margin-bottom: 16px;
  .stat {
    .label { display: block; font-size: 12px; color: var(--z-text-secondary); }
    .value { font-size: 18px; font-weight: 600; }
  }
}
@media (max-width: 1024px) {
  .content-grid { grid-template-columns: 1fr; }
}
</style>
