<script setup lang="ts">
import { ref, onMounted, watch } from 'vue'
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
} from 'chart.js'

ChartJS.register(CategoryScale, LinearScale, PointElement, LineElement, Title, Tooltip, Legend)

const props = defineProps<{ strategyId: string }>()

const chartData = ref({
  labels: [] as string[],
  datasets: [
    { label: 'Cumulative PnL', data: [] as number[], borderColor: '#0ea5e9', tension: 0.4 },
  ],
})

const chartOptions = {
  responsive: true,
  maintainAspectRatio: false,
  plugins: { legend: { display: false } },
  scales: {
    y: { grid: { color: 'rgba(0, 0, 0, 0.05)' } },
    x: { grid: { display: false } },
  },
}

function generateData() {
  const labels: string[] = []
  const data: number[] = []
  let pnl = 0
  for (let i = 13; i >= 0; i--) {
    const date = new Date()
    date.setDate(date.getDate() - i)
    labels.push(date.toLocaleDateString('en-US', { month: 'short', day: 'numeric' }))
    pnl += (Math.random() - 0.3) * 500
    data.push(Math.round(pnl * 100) / 100)
  }
  chartData.value.labels = labels
  chartData.value.datasets[0].data = data
}

onMounted(generateData)
watch(() => props.strategyId, generateData)
</script>

<template>
  <div class="chart-container"><Line :data="chartData" :options="chartOptions" /></div>
</template>

<style scoped>
.chart-container { height: 200px; }
</style>
