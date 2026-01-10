<script setup lang="ts">
import { ref } from 'vue'
import { useRouter } from 'vue-router'
import { useThemeStore, useAuthStore } from '@/stores'
import {
  House, DataAnalysis, Coin, List, Clock, Warning, Setting, Document, Moon, Sunny
} from '@element-plus/icons-vue'

const router = useRouter()
const themeStore = useThemeStore()
const authStore = useAuthStore()
const isCollapsed = ref(false)

const menuItems = [
  { path: '/', icon: House, title: 'Dashboard' },
  { path: '/strategies', icon: DataAnalysis, title: 'Strategies' },
  { path: '/positions', icon: Coin, title: 'Positions' },
  { path: '/orders', icon: List, title: 'Orders' },
  { path: '/trades', icon: Clock, title: 'Trades' },
  { path: '/risk', icon: Warning, title: 'Risk' },
  { path: '/settings', icon: Setting, title: 'Settings' },
  { path: '/logs', icon: Document, title: 'Logs' },
]

function handleLogout() {
  authStore.logout()
  router.push('/login')
}
</script>

<template>
  <el-container class="h-screen">
    <el-aside :width="isCollapsed ? '64px' : '200px'" class="sidebar">
      <div class="logo" :class="{ collapsed: isCollapsed }">
        <span v-if="!isCollapsed">Zephyr</span>
        <span v-else>Z</span>
      </div>
      <el-menu :collapse="isCollapsed" :default-active="$route.path" router>
        <el-menu-item v-for="item in menuItems" :key="item.path" :index="item.path">
          <el-icon><component :is="item.icon" /></el-icon>
          <template #title>{{ item.title }}</template>
        </el-menu-item>
      </el-menu>
    </el-aside>

    <el-container>
      <el-header class="header">
        <div class="header-left">
          <el-button :icon="isCollapsed ? 'Expand' : 'Fold'" text @click="isCollapsed = !isCollapsed" />
        </div>
        <div class="header-right">
          <el-button :icon="themeStore.isDark ? Sunny : Moon" circle @click="themeStore.toggleTheme" />
          <el-dropdown trigger="click">
            <el-avatar :size="32" class="cursor-pointer">U</el-avatar>
            <template #dropdown>
              <el-dropdown-menu>
                <el-dropdown-item @click="handleLogout">Logout</el-dropdown-item>
              </el-dropdown-menu>
            </template>
          </el-dropdown>
        </div>
      </el-header>
      <el-main class="main-content">
        <router-view />
      </el-main>
    </el-container>
  </el-container>
</template>

<style scoped lang="scss">
.sidebar {
  background-color: var(--z-bg-primary);
  border-right: 1px solid var(--z-border-color);
  transition: width 0.3s;
}
.logo {
  height: 60px;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 24px;
  font-weight: bold;
  color: var(--el-color-primary);
  &.collapsed { font-size: 20px; }
}
.header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  background-color: var(--z-bg-primary);
  border-bottom: 1px solid var(--z-border-color);
}
.header-right {
  display: flex;
  align-items: center;
  gap: 12px;
}
.main-content {
  background-color: var(--z-bg-secondary);
  overflow-y: auto;
}
</style>
