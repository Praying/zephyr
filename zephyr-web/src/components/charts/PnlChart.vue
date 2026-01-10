<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { Line } from 'vue-chartjs'
import {
  Chart as ChartJS,
  CategoryScale,
  LinearScale,
  PointElement,
  LineElement,
  Title,
  Tooltip,
  Legend,
  Filler,
} from 'chart.js'

ChartJS.register(CategoryScale, LinearScale, PointElement, LineElement, Title, Tooltip, Legend, Filler)

const chartData = ref({
  labels: [] as string[],
  datasets: [
    {
      label: 'PnL',
      data: [] as number[],
      borderColor: '#0ea5e9',
      backgroundColor: 'rgba(14, 165, 233, 0.1)',
      fill: true,
      tension: 0.4,
    },
  ],
})

const chartOptions = {
  responsive: true,
  maintainAspectRatio: false,
  plugins: {
    legend: { display: false },
    tooltip: {
      callbacks: {
        label: (ctx: { parsed: { y: number } }) => `$${ctx.parsed.y.toFixed(2)}`,
      },
    },
  },
  scales: {
    y: {
      grid: { color: 'rgba(0, 0, 0, 0.05)' },
      ticks: { callback: (value: number) => `$${value}` },
    },
    x: { grid: { display: false } },
  },
}

onMounted(() => {
  // Generate mock data for last 30 days
  const labels: string[] = []
  const data: number[] = []
  let pnl = 100000
  for (let i = 29; i >= 0; i--) {
    const date = new Date()
    date.setDate(date.getDate() - i)
    labels.push(date.toLocaleDateString('en-US', { month: 'short', day: 'numeric' }))
    pnl += (Math.random() - 0.4) * 2000
    data.push(Math.round(pnl * 100) / 100)
  }
  chartData.value.labels = labels
  chartData.value.datasets[0].data = data
})
</script>

<template>
  <div class="chart-container">
    <Line :data="chartData" :options="chartOptions" />
  </div>
</template>

<style scoped>
.chart-container { height: 300px; }
</style>
