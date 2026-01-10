<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useTradingStore } from '@/stores/trading'
import { TrendCharts, Coin, List, DataAnalysis } from '@element-plus/icons-vue'
import PnlChart from '@/components/charts/PnlChart.vue'
import PositionPieChart from '@/components/charts/PositionPieChart.vue'

const tradingStore = useTradingStore()

const stats = computed(() => [
  {
    title: 'Total Equity',
    value: tradingStore.totalEquity,
    format: 'currency',
    icon: Coin,
    color: '#0ea5e9',
  },
  {
    title: 'Unrealized PnL',
    value: tradingStore.totalPnl,
    format: 'currency',
    icon: TrendCharts,
    color: tradingStore.totalPnl >= 0 ? '#10b981' : '#ef4444',
  },
  {
    title: 'Active Positions',
    value: tradingStore.positions.length,
    format: 'number',
    icon: List,
    color: '#f59e0b',
  },
  {
    title: 'Active Strategies',
    value: tradingStore.activeStrategies.length,
    format: 'number',
    icon: DataAnalysis,
    color: '#8b5cf6',
  },
])

function formatValue(value: number, format: string): string {
  if (format === 'currency') {
    return new Intl.NumberFormat('en-US', {
      style: 'currency',
      currency: 'USD',
      minimumFractionDigits: 2,
    }).format(value)
  }
  return value.toString()
}

// Mock data for demo
onMounted(() => {
  tradingStore.updateAccount({
    exchange: 'binance',
    totalEquity: 125430.56,
    availableBalance: 85000,
    marginUsed: 40430.56,
    unrealizedPnl: 3250.78,
    balances: [{ currency: 'USDT', total: 125430.56, available: 85000, frozen: 40430.56 }],
    updateTime: Date.now(),
  })
  tradingStore.updatePositions([
    { symbol: 'BTCUSDT', exchange: 'binance', side: 'long', quantity: 0.5, entryPrice: 42000, markPrice: 43500, liquidationPrice: 35000, unrealizedPnl: 750, realizedPnl: 0, leverage: 10, marginType: 'cross', updateTime: Date.now() },
    { symbol: 'ETHUSDT', exchange: 'binance', side: 'long', quantity: 5, entryPrice: 2200, markPrice: 2350, liquidationPrice: 1800, unrealizedPnl: 750, realizedPnl: 0, leverage: 5, marginType: 'isolated', updateTime: Date.now() },
    { symbol: 'SOLUSDT', exchange: 'okx', side: 'short', quantity: 100, entryPrice: 95, markPrice: 92, liquidationPrice: 120, unrealizedPnl: 300, realizedPnl: 0, leverage: 3, marginType: 'cross', updateTime: Date.now() },
  ])
  tradingStore.updateStrategies([
    { id: '1', name: 'BTC Trend', type: 'cta', status: 'running', symbols: ['BTCUSDT'], pnl: 5230, trades: 45, winRate: 0.62, sharpeRatio: 1.8, maxDrawdown: 0.08, startTime: Date.now() - 86400000 * 7 },
    { id: '2', name: 'ETH Grid', type: 'cta', status: 'running', symbols: ['ETHUSDT'], pnl: 2100, trades: 120, winRate: 0.55, sharpeRatio: 1.2, maxDrawdown: 0.05, startTime: Date.now() - 86400000 * 3 },
  ])
})
</script>

<template>
  <div class="dashboard">
    <h1 class="page-title">Dashboard</h1>
    <div class="stats-grid">
      <div v-for="stat in stats" :key="stat.title" class="stat-card card">
        <div class="stat-icon" :style="{ backgroundColor: stat.color + '20', color: stat.color }">
          <el-icon :size="24"><component :is="stat.icon" /></el-icon>
        </div>
        <div class="stat-content">
          <div class="stat-title">{{ stat.title }}</div>
          <div class="stat-value" :class="{ profit: stat.value > 0 && stat.format === 'currency', loss: stat.value < 0 }">
            {{ formatValue(stat.value, stat.format) }}
          </div>
        </div>
      </div>
    </div>
    <div class="charts-grid">
      <div class="card chart-card"><h3>PnL History</h3><PnlChart /></div>
      <div class="card chart-card"><h3>Position Distribution</h3><PositionPieChart /></div>
    </div>
  </div>
</template>

<style scoped lang="scss">
.dashboard {
  padding: 20px;
}
.page-title {
  font-size: 24px;
  font-weight: 600;
  margin-bottom: 20px;
}
.stats-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(250px, 1fr));
  gap: 16px;
  margin-bottom: 24px;
}
.stat-card {
  display: flex;
  align-items: center;
  gap: 16px;
}
.stat-icon {
  width: 48px;
  height: 48px;
  border-radius: 12px;
  display: flex;
  align-items: center;
  justify-content: center;
}
.stat-title {
  font-size: 14px;
  color: var(--z-text-secondary);
}
.stat-value {
  font-size: 24px;
  font-weight: 600;
}
.charts-grid {
  display: grid;
  grid-template-columns: 2fr 1fr;
  gap: 16px;
}
.chart-card {
  h3 {
    margin-bottom: 16px;
    font-size: 16px;
    font-weight: 600;
  }
}
@media (max-width: 1024px) {
  .charts-grid {
    grid-template-columns: 1fr;
  }
}
</style>
