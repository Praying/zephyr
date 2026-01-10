<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useTradingStore } from '@/stores/trading'
import { ElMessage, ElMessageBox } from 'element-plus'
import type { Order } from '@/types'

const tradingStore = useTradingStore()
const showOrderForm = ref(false)

const orders = computed(() => tradingStore.orders)

const orderForm = ref({
  symbol: '',
  side: 'buy' as 'buy' | 'sell',
  type: 'limit' as 'limit' | 'market',
  price: null as number | null,
  quantity: null as number | null,
})

function getStatusType(status: string) {
  const map: Record<string, string> = {
    new: 'primary', partially_filled: 'warning', filled: 'success',
    canceled: 'info', rejected: 'danger', pending: 'info',
  }
  return map[status] || 'info'
}

function formatTime(ts: number): string {
  return new Date(ts).toLocaleString()
}

async function cancelOrder(order: Order) {
  try {
    await ElMessageBox.confirm(`Cancel order ${order.orderId}?`, 'Confirm', { type: 'warning' })
    const updated = orders.value.map((o) =>
      o.orderId === order.orderId ? { ...o, status: 'canceled' as const } : o
    )
    tradingStore.updateOrders(updated)
    ElMessage.success('Order cancelled')
  } catch {}
}

async function submitOrder() {
  if (!orderForm.value.symbol || !orderForm.value.quantity) {
    ElMessage.warning('Please fill required fields')
    return
  }
  const newOrder: Order = {
    orderId: `ORD${Date.now()}`,
    symbol: orderForm.value.symbol,
    exchange: 'binance',
    side: orderForm.value.side,
    type: orderForm.value.type,
    status: 'new',
    price: orderForm.value.price,
    quantity: orderForm.value.quantity!,
    filledQuantity: 0,
    avgPrice: null,
    createTime: Date.now(),
    updateTime: Date.now(),
  }
  tradingStore.updateOrders([newOrder, ...orders.value])
  showOrderForm.value = false
  ElMessage.success('Order submitted')
}

onMounted(() => {
  tradingStore.updateOrders([
    { orderId: 'ORD001', symbol: 'BTCUSDT', exchange: 'binance', side: 'buy', type: 'limit', status: 'new', price: 42000, quantity: 0.1, filledQuantity: 0, avgPrice: null, createTime: Date.now() - 3600000, updateTime: Date.now() - 3600000 },
    { orderId: 'ORD002', symbol: 'ETHUSDT', exchange: 'binance', side: 'sell', type: 'limit', status: 'partially_filled', price: 2400, quantity: 2, filledQuantity: 1, avgPrice: 2400, createTime: Date.now() - 7200000, updateTime: Date.now() - 1800000 },
  ])
})
</script>

<template>
  <div class="orders-page">
    <div class="page-header">
      <h1 class="page-title">Orders</h1>
      <el-button type="primary" @click="showOrderForm = true">New Order</el-button>
    </div>
    <div class="card">
      <el-table :data="orders" stripe>
        <el-table-column prop="orderId" label="Order ID" width="140" />
        <el-table-column prop="symbol" label="Symbol" width="100" />
        <el-table-column prop="side" label="Side" width="80">
          <template #default="{ row }">
            <el-tag :type="row.side === 'buy' ? 'success' : 'danger'" size="small">{{ row.side.toUpperCase() }}</el-tag>
          </template>
        </el-table-column>
        <el-table-column prop="type" label="Type" width="80" />
        <el-table-column prop="price" label="Price" width="100">
          <template #default="{ row }">{{ row.price ? `$${row.price}` : 'Market' }}</template>
        </el-table-column>
        <el-table-column prop="quantity" label="Qty" width="80" />
        <el-table-column prop="filledQuantity" label="Filled" width="80" />
        <el-table-column prop="status" label="Status" width="120">
          <template #default="{ row }">
            <el-tag :type="getStatusType(row.status)" size="small">{{ row.status }}</el-tag>
          </template>
        </el-table-column>
        <el-table-column prop="createTime" label="Time" width="160">
          <template #default="{ row }">{{ formatTime(row.createTime) }}</template>
        </el-table-column>
        <el-table-column label="Actions" width="100" fixed="right">
          <template #default="{ row }">
            <el-button v-if="['new', 'partially_filled'].includes(row.status)" type="danger" size="small" @click="cancelOrder(row)">Cancel</el-button>
          </template>
        </el-table-column>
      </el-table>
    </div>

    <el-dialog v-model="showOrderForm" title="New Order" width="400px">
      <el-form label-position="top">
        <el-form-item label="Symbol" required>
          <el-input v-model="orderForm.symbol" placeholder="e.g. BTCUSDT" />
        </el-form-item>
        <el-form-item label="Side">
          <el-radio-group v-model="orderForm.side">
            <el-radio-button value="buy">Buy</el-radio-button>
            <el-radio-button value="sell">Sell</el-radio-button>
          </el-radio-group>
        </el-form-item>
        <el-form-item label="Type">
          <el-radio-group v-model="orderForm.type">
            <el-radio-button value="limit">Limit</el-radio-button>
            <el-radio-button value="market">Market</el-radio-button>
          </el-radio-group>
        </el-form-item>
        <el-form-item v-if="orderForm.type === 'limit'" label="Price">
          <el-input-number v-model="orderForm.price" :min="0" :precision="2" style="width: 100%" />
        </el-form-item>
        <el-form-item label="Quantity" required>
          <el-input-number v-model="orderForm.quantity" :min="0" :precision="4" style="width: 100%" />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="showOrderForm = false">Cancel</el-button>
        <el-button type="primary" @click="submitOrder">Submit</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<style scoped lang="scss">
.orders-page { padding: 20px; }
.page-header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 20px; }
.page-title { font-size: 24px; font-weight: 600; margin: 0; }
</style>
