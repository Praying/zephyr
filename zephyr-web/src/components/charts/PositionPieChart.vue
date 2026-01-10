<script setup lang="ts">
import { computed } from 'vue'
import { Pie } from 'vue-chartjs'
import { Chart as ChartJS, ArcElement, Tooltip, Legend } from 'chart.js'
import { useTradingStore } from '@/stores/trading'

ChartJS.register(ArcElement, Tooltip, Legend)

const tradingStore = useTradingStore()

const chartData = computed(() => {
  const positions = tradingStore.positions
  const labels = positions.map((p) => p.symbol)
  const data = positions.map((p) => Math.abs(p.quantity * p.markPrice))
  const colors = ['#0ea5e9', '#10b981', '#f59e0b', '#ef4444', '#8b5cf6', '#ec4899']

  return {
    labels,
    datasets: [
      {
        data,
        backgroundColor: colors.slice(0, positions.length),
        borderWidth: 0,
      },
    ],
  }
})

const chartOptions = {
  responsive: true,
  maintainAspectRatio: false,
  plugins: {
    legend: {
      position: 'bottom' as const,
      labels: { padding: 16, usePointStyle: true },
    },
    tooltip: {
      callbacks: {
        label: (ctx: { label: string; parsed: number }) =>
          `${ctx.label}: $${ctx.parsed.toLocaleString()}`,
      },
    },
  },
}
</script>

<template>
  <div class="chart-container">
    <Pie v-if="tradingStore.positions.length > 0" :data="chartData" :options="chartOptions" />
    <div v-else class="no-data">No positions</div>
  </div>
</template>

<style scoped>
.chart-container { height: 300px; display: flex; align-items: center; justify-content: center; }
.no-data { color: var(--z-text-secondary); }
</style>
