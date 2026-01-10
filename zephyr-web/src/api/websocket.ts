import { ref } from 'vue'
import { useTradingStore } from '@/stores/trading'
import { useAuthStore } from '@/stores/auth'

type MessageHandler = (data: unknown) => void

class WebSocketService {
  private ws: WebSocket | null = null
  private reconnectAttempts = 0
  private maxReconnectAttempts = 5
  private reconnectDelay = 1000
  private handlers: Map<string, MessageHandler[]> = new Map()
  public isConnected = ref(false)

  connect() {
    const authStore = useAuthStore()
    if (!authStore.token) return

    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:'
    const url = `${protocol}//${window.location.host}/ws?token=${authStore.token}`

    this.ws = new WebSocket(url)
    this.ws.onopen = () => {
      this.isConnected.value = true
      this.reconnectAttempts = 0
      this.subscribe(['positions', 'orders', 'trades', 'account'])
    }
    this.ws.onclose = () => {
      this.isConnected.value = false
      this.attemptReconnect()
    }
    this.ws.onmessage = (event) => this.handleMessage(event)
    this.ws.onerror = (error) => console.error('WebSocket error:', error)
  }

  private handleMessage(event: MessageEvent) {
    try {
      const msg = JSON.parse(event.data)
      const tradingStore = useTradingStore()
      switch (msg.type) {
        case 'positions': tradingStore.updatePositions(msg.data); break
        case 'orders': tradingStore.updateOrders(msg.data); break
        case 'trade': tradingStore.addTrade(msg.data); break
        case 'account': tradingStore.updateAccount(msg.data); break
      }
      this.handlers.get(msg.type)?.forEach((h) => h(msg.data))
    } catch (e) { console.error('Failed to parse message:', e) }
  }

  private attemptReconnect() {
    if (this.reconnectAttempts >= this.maxReconnectAttempts) return
    this.reconnectAttempts++
    setTimeout(() => this.connect(), this.reconnectDelay * this.reconnectAttempts)
  }

  subscribe(channels: string[]) {
    this.send({ type: 'subscribe', channels })
  }

  unsubscribe(channels: string[]) {
    this.send({ type: 'unsubscribe', channels })
  }

  send(data: unknown) {
    if (this.ws?.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify(data))
    }
  }

  on(type: string, handler: MessageHandler) {
    if (!this.handlers.has(type)) this.handlers.set(type, [])
    this.handlers.get(type)!.push(handler)
  }

  off(type: string, handler: MessageHandler) {
    const handlers = this.handlers.get(type)
    if (handlers) {
      const index = handlers.indexOf(handler)
      if (index > -1) handlers.splice(index, 1)
    }
  }

  disconnect() {
    this.ws?.close()
    this.ws = null
    this.isConnected.value = false
  }
}

export const wsService = new WebSocketService()
