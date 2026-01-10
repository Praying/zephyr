export interface User {
  id: string
  username: string
  email: string
  role: 'admin' | 'trader' | 'viewer'
}

export interface Position {
  symbol: string
  exchange: string
  side: 'long' | 'short'
  quantity: number
  entryPrice: number
  markPrice: number
  liquidationPrice: number | null
  unrealizedPnl: number
  realizedPnl: number
  leverage: number
  marginType: 'cross' | 'isolated'
  updateTime: number
}

export interface Order {
  orderId: string
  clientOrderId?: string
  symbol: string
  exchange: string
  side: 'buy' | 'sell'
  type: 'limit' | 'market' | 'stop_limit' | 'stop_market'
  status: 'pending' | 'new' | 'partially_filled' | 'filled' | 'canceled' | 'rejected'
  price: number | null
  quantity: number
  filledQuantity: number
  avgPrice: number | null
  createTime: number
  updateTime: number
}


export interface Strategy {
  id: string
  name: string
  type: 'cta' | 'hft' | 'uft'
  status: 'running' | 'stopped' | 'paused' | 'error'
  symbols: string[]
  pnl: number
  trades: number
  winRate: number
  sharpeRatio: number
  maxDrawdown: number
  startTime: number | null
}

export interface Account {
  exchange: string
  totalEquity: number
  availableBalance: number
  marginUsed: number
  unrealizedPnl: number
  balances: Balance[]
  updateTime: number
}

export interface Balance {
  currency: string
  total: number
  available: number
  frozen: number
}

export interface Trade {
  tradeId: string
  orderId: string
  symbol: string
  exchange: string
  side: 'buy' | 'sell'
  price: number
  quantity: number
  fee: number
  feeCurrency: string
  time: number
}

export interface RiskLimit {
  type: 'position' | 'order_rate' | 'daily_loss'
  symbol?: string
  limit: number
  current: number
  status: 'normal' | 'warning' | 'breached'
}

export interface LogEntry {
  timestamp: number
  level: 'debug' | 'info' | 'warn' | 'error'
  component: string
  message: string
  data?: Record<string, unknown>
}
