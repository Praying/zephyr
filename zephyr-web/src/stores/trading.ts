import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import type { Position, Order, Strategy, Account, Trade } from '@/types'

export const useTradingStore = defineStore('trading', () => {
  const positions = ref<Position[]>([])
  const orders = ref<Order[]>([])
  const strategies = ref<Strategy[]>([])
  const account = ref<Account | null>(null)
  const trades = ref<Trade[]>([])

  const totalPnl = computed(() => positions.value.reduce((sum, p) => sum + p.unrealizedPnl, 0))
  const totalEquity = computed(() => account.value?.totalEquity ?? 0)
  const activeStrategies = computed(() => strategies.value.filter((s) => s.status === 'running'))

  function updatePositions(newPositions: Position[]) {
    positions.value = newPositions
  }

  function updateOrders(newOrders: Order[]) {
    orders.value = newOrders
  }

  function updateStrategies(newStrategies: Strategy[]) {
    strategies.value = newStrategies
  }

  function updateAccount(newAccount: Account) {
    account.value = newAccount
  }

  function addTrade(trade: Trade) {
    trades.value.unshift(trade)
    if (trades.value.length > 1000) trades.value.pop()
  }

  return {
    positions, orders, strategies, account, trades,
    totalPnl, totalEquity, activeStrategies,
    updatePositions, updateOrders, updateStrategies, updateAccount, addTrade,
  }
})
