import { get, post, del } from './client'
import type { Position, Order, Strategy, Account, Trade, RiskLimit } from '@/types'

// Strategies
export const getStrategies = () => get<Strategy[]>('/strategies')
export const startStrategy = (id: string) => post(`/strategies/${id}/start`)
export const stopStrategy = (id: string) => post(`/strategies/${id}/stop`)

// Positions
export const getPositions = () => get<Position[]>('/positions')

// Orders
export const getOrders = () => get<Order[]>('/orders')
export const submitOrder = (order: Partial<Order>) => post<Order>('/orders', order)
export const cancelOrder = (id: string) => del(`/orders/${id}`)

// Trades
export const getTrades = (params?: { symbol?: string; limit?: number }) =>
  get<Trade[]>('/trades', { params })

// Account
export const getAccount = () => get<Account>('/account')

// Risk
export const getRiskLimits = () => get<RiskLimit[]>('/risk/limits')
export const updateRiskLimit = (limit: RiskLimit) => post('/risk/limits', limit)

// Health
export const getHealth = () => get<{ status: string }>('/health')
