<script setup lang="ts">
import { computed } from 'vue'
import { useTradingStore } from '@/stores/trading'
import { ElMessage, ElMessageBox } from 'element-plus'

const tradingStore = useTradingStore()
const positions = computed(() => tradingStore.positions)

function formatPrice(value: number): string {
  return value.toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 })
}

function formatPnl(value: number): string {
  const prefix = value >= 0 ? '+' : ''
  return `${prefix}$${formatPrice(value)}`
}

function getSideType(side: string) {
  return side === 'long' ? 'success' : 'danger'
}

async function closePosition(symbol: string) {
  try {
    await ElMessageBox.confirm(
      `Are you sure you want to close the ${symbol} position?`,
      'Close Position',
      { confirmButtonText: 'Close', cancelButtonText: 'Cancel', type: 'warning' }
    )
    // Mock close - would call API
    const updated = positions.value.filter((p) => p.symbol !== symbol)
    tradingStore.updatePositions(updated)
    ElMessage.success(`Position ${symbol} closed`)
  } catch {
    // User cancelled
  }
}
</script>

<template>
  <div class="positions-page">
    <h1 class="page-title">Positions</h1>
    <div class="card">
      <el-table :data="positions" stripe>
        <el-table-column prop="symbol" label="Symbol" width="120" />
        <el-table-column prop="exchange" label="Exchange" width="100" />
        <el-table-column prop="side" label="Side" width="80">
          <template #default="{ row }">
            <el-tag :type="getSideType(row.side)" size="small">{{ row.side.toUpperCase() }}</el-tag>
          </template>
        </el-table-column>
        <el-table-column prop="quantity" label="Quantity" width="100">
          <template #default="{ row }">{{ row.quantity }}</template>
        </el-table-column>
        <el-table-column prop="entryPrice" label="Entry Price" width="120">
          <template #default="{ row }">${{ formatPrice(row.entryPrice) }}</template>
        </el-table-column>
        <el-table-column prop="markPrice" label="Mark Price" width="120">
          <template #default="{ row }">${{ formatPrice(row.markPrice) }}</template>
        </el-table-column>
        <el-table-column prop="unrealizedPnl" label="Unrealized PnL" width="140">
          <template #default="{ row }">
            <span :class="row.unrealizedPnl >= 0 ? 'profit' : 'loss'">
              {{ formatPnl(row.unrealizedPnl) }}
            </span>
          </template>
        </el-table-column>
        <el-table-column prop="leverage" label="Leverage" width="80">
          <template #default="{ row }">{{ row.leverage }}x</template>
        </el-table-column>
        <el-table-column prop="marginType" label="Margin" width="80">
          <template #default="{ row }">
            <el-tag size="small" type="info">{{ row.marginType }}</el-tag>
          </template>
        </el-table-column>
        <el-table-column prop="liquidationPrice" label="Liq. Price" width="120">
          <template #default="{ row }">
            <span class="loss">${{ row.liquidationPrice ? formatPrice(row.liquidationPrice) : '-' }}</span>
          </template>
        </el-table-column>
        <el-table-column label="Actions" width="100" fixed="right">
          <template #default="{ row }">
            <el-button type="danger" size="small" @click="closePosition(row.symbol)">Close</el-button>
          </template>
        </el-table-column>
      </el-table>
    </div>
  </div>
</template>

<style scoped lang="scss">
.positions-page { padding: 20px; }
.page-title { font-size: 24px; font-weight: 600; margin-bottom: 20px; }
</style>
