<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useTradingStore } from '@/stores/trading'
import type { Trade } from '@/types'

const tradingStore = useTradingStore()
const filterSymbol = ref('')

const trades = computed(() => {
  if (!filterSymbol.value) return tradingStore.trades
  return tradingStore.trades.filter((t) => t.symbol.includes(filterSymbol.value.toUpperCase()))
})

function formatTime(ts: number): string {
  return new Date(ts).toLocaleString()
}

function exportTrades() {
  const csv = [
    ['Trade ID', 'Order ID', 'Symbol', 'Side', 'Price', 'Quantity', 'Fee', 'Time'].join(','),
    ...trades.value.map((t) =>
      [t.tradeId, t.orderId, t.symbol, t.side, t.price, t.quantity, `${t.fee} ${t.feeCurrency}`, formatTime(t.time)].join(',')
    ),
  ].join('\n')
  const blob = new Blob([csv], { type: 'text/csv' })
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = `trades_${new Date().toISOString().split('T')[0]}.csv`
  a.click()
  URL.revokeObjectURL(url)
}

onMounted(() => {
  const mockTrades: Trade[] = [
    { tradeId: 'T001', orderId: 'ORD001', symbol: 'BTCUSDT', exchange: 'binance', side: 'buy', price: 42150, quantity: 0.1, fee: 0.42, feeCurrency: 'USDT', time: Date.now() - 3600000 },
    { tradeId: 'T002', orderId: 'ORD002', symbol: 'ETHUSDT', exchange: 'binance', side: 'sell', price: 2400, quantity: 1, fee: 2.4, feeCurrency: 'USDT', time: Date.now() - 7200000 },
    { tradeId: 'T003', orderId: 'ORD003', symbol: 'BTCUSDT', exchange: 'okx', side: 'sell', price: 43000, quantity: 0.05, fee: 0.22, feeCurrency: 'USDT', time: Date.now() - 86400000 },
  ]
  mockTrades.forEach((t) => tradingStore.addTrade(t))
})
</script>

<template>
  <div class="trades-page">
    <div class="page-header">
      <h1 class="page-title">Trade History</h1>
      <div class="header-actions">
        <el-input v-model="filterSymbol" placeholder="Filter by symbol" clearable style="width: 200px" />
        <el-button @click="exportTrades">Export CSV</el-button>
      </div>
    </div>
    <div class="card">
      <el-table :data="trades" stripe>
        <el-table-column prop="tradeId" label="Trade ID" width="120" />
        <el-table-column prop="orderId" label="Order ID" width="120" />
        <el-table-column prop="symbol" label="Symbol" width="100" />
        <el-table-column prop="exchange" label="Exchange" width="100" />
        <el-table-column prop="side" label="Side" width="80">
          <template #default="{ row }">
            <el-tag :type="row.side === 'buy' ? 'success' : 'danger'" size="small">{{ row.side.toUpperCase() }}</el-tag>
          </template>
        </el-table-column>
        <el-table-column prop="price" label="Price" width="120">
          <template #default="{ row }">${{ row.price.toLocaleString() }}</template>
        </el-table-column>
        <el-table-column prop="quantity" label="Quantity" width="100" />
        <el-table-column prop="fee" label="Fee" width="120">
          <template #default="{ row }">{{ row.fee }} {{ row.feeCurrency }}</template>
        </el-table-column>
        <el-table-column prop="time" label="Time" width="180">
          <template #default="{ row }">{{ formatTime(row.time) }}</template>
        </el-table-column>
      </el-table>
    </div>
  </div>
</template>

<style scoped lang="scss">
.trades-page { padding: 20px; }
.page-header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 20px; }
.page-title { font-size: 24px; font-weight: 600; margin: 0; }
.header-actions { display: flex; gap: 12px; }
</style>
