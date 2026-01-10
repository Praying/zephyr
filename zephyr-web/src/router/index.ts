import { createRouter, createWebHistory } from 'vue-router'
import type { RouteRecordRaw } from 'vue-router'

const routes: RouteRecordRaw[] = [
  {
    path: '/',
    component: () => import('@/layouts/MainLayout.vue'),
    children: [
      { path: '', name: 'dashboard', component: () => import('@/views/Dashboard.vue') },
      { path: 'strategies', name: 'strategies', component: () => import('@/views/Strategies.vue') },
      { path: 'positions', name: 'positions', component: () => import('@/views/Positions.vue') },
      { path: 'orders', name: 'orders', component: () => import('@/views/Orders.vue') },
      { path: 'trades', name: 'trades', component: () => import('@/views/Trades.vue') },
      { path: 'risk', name: 'risk', component: () => import('@/views/Risk.vue') },
      { path: 'settings', name: 'settings', component: () => import('@/views/Settings.vue') },
      { path: 'logs', name: 'logs', component: () => import('@/views/Logs.vue') },
    ],
  },
  { path: '/login', name: 'login', component: () => import('@/views/Login.vue') },
]

const router = createRouter({
  history: createWebHistory(),
  routes,
})

export default router
