<script setup lang="ts">
import { ref } from 'vue'
import { useThemeStore } from '@/stores/theme'
import { ElMessage } from 'element-plus'

const themeStore = useThemeStore()

const config = ref({
  general: {
    timezone: 'UTC',
    language: 'en',
  },
  trading: {
    defaultLeverage: 10,
    defaultMarginType: 'cross',
    confirmOrders: true,
  },
  notifications: {
    orderFills: true,
    riskAlerts: true,
    systemAlerts: true,
    email: '',
  },
  api: {
    rateLimit: 100,
    timeout: 30000,
  },
})

const timezones = ['UTC', 'America/New_York', 'Europe/London', 'Asia/Tokyo', 'Asia/Shanghai']
const languages = [{ value: 'en', label: 'English' }, { value: 'zh', label: '中文' }]

function saveSettings() {
  // Mock save - would call API
  ElMessage.success('Settings saved successfully')
}

function resetSettings() {
  ElMessage.info('Settings reset to defaults')
}
</script>

<template>
  <div class="settings-page">
    <h1 class="page-title">Settings</h1>
    <div class="settings-grid">
      <div class="card settings-section">
        <h3>General</h3>
        <el-form label-position="top">
          <el-form-item label="Theme">
            <el-switch v-model="themeStore.isDark" active-text="Dark" inactive-text="Light" @change="themeStore.toggleTheme" />
          </el-form-item>
          <el-form-item label="Timezone">
            <el-select v-model="config.timezone" style="width: 100%">
              <el-option v-for="tz in timezones" :key="tz" :label="tz" :value="tz" />
            </el-select>
          </el-form-item>
          <el-form-item label="Language">
            <el-select v-model="config.general.language" style="width: 100%">
              <el-option v-for="lang in languages" :key="lang.value" :label="lang.label" :value="lang.value" />
            </el-select>
          </el-form-item>
        </el-form>
      </div>

      <div class="card settings-section">
        <h3>Trading</h3>
        <el-form label-position="top">
          <el-form-item label="Default Leverage">
            <el-slider v-model="config.trading.defaultLeverage" :min="1" :max="125" :marks="{ 1: '1x', 50: '50x', 125: '125x' }" />
          </el-form-item>
          <el-form-item label="Default Margin Type">
            <el-radio-group v-model="config.trading.defaultMarginType">
              <el-radio-button value="cross">Cross</el-radio-button>
              <el-radio-button value="isolated">Isolated</el-radio-button>
            </el-radio-group>
          </el-form-item>
          <el-form-item label="Confirm Orders">
            <el-switch v-model="config.trading.confirmOrders" />
          </el-form-item>
        </el-form>
      </div>

      <div class="card settings-section">
        <h3>Notifications</h3>
        <el-form label-position="top">
          <el-form-item label="Order Fills">
            <el-switch v-model="config.notifications.orderFills" />
          </el-form-item>
          <el-form-item label="Risk Alerts">
            <el-switch v-model="config.notifications.riskAlerts" />
          </el-form-item>
          <el-form-item label="System Alerts">
            <el-switch v-model="config.notifications.systemAlerts" />
          </el-form-item>
          <el-form-item label="Email Notifications">
            <el-input v-model="config.notifications.email" placeholder="your@email.com" />
          </el-form-item>
        </el-form>
      </div>

      <div class="card settings-section">
        <h3>API Settings</h3>
        <el-form label-position="top">
          <el-form-item label="Rate Limit (requests/min)">
            <el-input-number v-model="config.api.rateLimit" :min="10" :max="1000" style="width: 100%" />
          </el-form-item>
          <el-form-item label="Timeout (ms)">
            <el-input-number v-model="config.api.timeout" :min="1000" :max="60000" :step="1000" style="width: 100%" />
          </el-form-item>
        </el-form>
      </div>
    </div>

    <div class="settings-actions">
      <el-button @click="resetSettings">Reset to Defaults</el-button>
      <el-button type="primary" @click="saveSettings">Save Settings</el-button>
    </div>
  </div>
</template>

<style scoped lang="scss">
.settings-page { padding: 20px; }
.page-title { font-size: 24px; font-weight: 600; margin-bottom: 20px; }
.settings-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(350px, 1fr)); gap: 16px; }
.settings-section { h3 { margin-bottom: 16px; font-size: 16px; font-weight: 600; } }
.settings-actions { margin-top: 24px; display: flex; justify-content: flex-end; gap: 12px; }
</style>
