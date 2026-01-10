<script setup lang="ts">
import { useThemeStore } from '@/stores/theme'

const themeStore = useThemeStore()

// Initialize theme on app load
themeStore.initTheme()
</script>

<template>
  <el-config-provider :locale="undefined">
    <router-view />
  </el-config-provider>
</template>

<style>
#app {
  width: 100%;
  height: 100vh;
}
</style>
