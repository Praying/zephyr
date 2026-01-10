<script setup lang="ts">
import { ref, reactive } from 'vue'
import { useRouter } from 'vue-router'
import { useAuthStore } from '@/stores/auth'
import { ElMessage } from 'element-plus'

const router = useRouter()
const authStore = useAuthStore()
const loading = ref(false)

const form = reactive({
  username: '',
  password: '',
})

async function handleLogin() {
  if (!form.username || !form.password) {
    ElMessage.warning('Please enter username and password')
    return
  }
  loading.value = true
  try {
    // Mock login - replace with actual API call
    authStore.setToken('mock-jwt-token')
    authStore.setUser({ id: '1', username: form.username, email: '', role: 'trader' })
    router.push('/')
  } catch (error) {
    ElMessage.error('Login failed')
  } finally {
    loading.value = false
  }
}
</script>

<template>
  <div class="login-container">
    <div class="login-card">
      <h1 class="title">Zephyr</h1>
      <p class="subtitle">Crypto Trading Console</p>
      <el-form @submit.prevent="handleLogin">
        <el-form-item>
          <el-input v-model="form.username" placeholder="Username" size="large" />
        </el-form-item>
        <el-form-item>
          <el-input v-model="form.password" type="password" placeholder="Password" size="large" show-password />
        </el-form-item>
        <el-button type="primary" size="large" :loading="loading" native-type="submit" class="w-full">
          Login
        </el-button>
      </el-form>
    </div>
  </div>
</template>

<style scoped lang="scss">
.login-container {
  min-height: 100vh;
  display: flex;
  align-items: center;
  justify-content: center;
  background: linear-gradient(135deg, #0ea5e9 0%, #0369a1 100%);
}
.login-card {
  width: 400px;
  padding: 40px;
  background: white;
  border-radius: 12px;
  box-shadow: 0 20px 40px rgba(0, 0, 0, 0.2);
}
.title {
  text-align: center;
  font-size: 32px;
  font-weight: bold;
  color: #0ea5e9;
  margin-bottom: 8px;
}
.subtitle {
  text-align: center;
  color: #606266;
  margin-bottom: 32px;
}
</style>
