<template>
  <div class="conn-list">
    <div class="head">
      <span>连接</span>
      <a-button size="small" type="primary" @click="openNew">+ 新建</a-button>
    </div>

    <a-list :data-source="list" size="small">
      <template #renderItem="{ item }">
        <a-list-item
          :class="{ active: item.id === activeId, row: true }"
          @click="activeId = item.id"
        >
          <a-badge :status="dot(item.id)" :text="item.name" />
          <template #actions>
            <a-button
              size="small"
              :type="isLive(item.id) ? 'default' : 'link'"
              @click.stop="toggle(item.id)"
            >
              {{ isLive(item.id) ? '断开' : '连接' }}
            </a-button>
            <a-dropdown @click.stop>
              <a-button size="small" type="text">⋯</a-button>
              <template #overlay>
                <a-menu>
                  <a-menu-item @click="openEdit(item)">编辑</a-menu-item>
                  <a-menu-item @click="cloneConn(item)">克隆</a-menu-item>
                  <a-menu-item @click="remove(item.id)" style="color:#ff4d4f">删除</a-menu-item>
                </a-menu>
              </template>
            </a-dropdown>
          </template>
        </a-list-item>
      </template>
    </a-list>

    <ConnectionForm
      v-model:open="formOpen"
      :connection="editing"
      @save="onSave"
    />
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { listen } from '@tauri-apps/api/event'
import ConnectionForm from './ConnectionForm.vue'
import { useConnectionsStore } from '../stores/connections'
import type { Connection, ConnectionState } from '../types'

const props = defineProps<{ activeId: string }>()
const emit = defineEmits<{ (e: 'update:activeId', v: string): void }>()

const connStore = useConnectionsStore()
const list = computed(() => connStore.list)
const activeId = computed({
  get: () => props.activeId,
  set: (v) => emit('update:activeId', v),
})

const formOpen = ref(false)
const editing = ref<Connection | null>(null)

function isLive(id: string) {
  const s = connStore.states[id]
  return s === 'connected' || s === 'reconnecting' || s === 'connecting'
}
function dot(id: string): 'success' | 'processing' | 'default' | 'error' | 'warning' {
  const s = connStore.states[id] as ConnectionState | undefined
  const map: Record<ConnectionState, 'success' | 'processing' | 'default' | 'error' | 'warning'> = {
    idle: 'default',
    connecting: 'processing',
    connected: 'success',
    reconnecting: 'warning',
    failed: 'error',
    disconnecting: 'default',
  }
  return map[s ?? 'idle']
}
function toggle(id: string) {
  if (isLive(id)) connStore.disconnect(id)
  else connStore.connect(id)
}

function openNew() {
  editing.value = null
  formOpen.value = true
}
function openEdit(c: Connection) {
  editing.value = { ...c }
  formOpen.value = true
}
function cloneConn(c: Connection) {
  const clone: Connection = {
    ...JSON.parse(JSON.stringify(c)),
    id: crypto.randomUUID(),
    name: c.name + '（副本）',
    client_id: 'mqttkit-' + Math.random().toString(16).slice(2, 10),
    credential_ref: null,
  }
  editing.value = clone
  formOpen.value = true
}
async function remove(id: string) {
  await connStore.remove(id)
  if (activeId.value === id) activeId.value = ''
}
async function onSave(conn: Connection, password?: string) {
  await connStore.save(conn, password)
  if (!activeId.value) activeId.value = conn.id
}

// 菜单"新建连接"事件
let unlisten: (() => void) | null = null
onMounted(async () => {
  unlisten = await listen('menu:new_conn', () => openNew())
})
onBeforeUnmount(() => unlisten?.())

defineExpose({ openNew })
</script>

<style scoped>
.conn-list { display: flex; flex-direction: column; height: 100%; }
.head {
  display: flex; justify-content: space-between; align-items: center;
  padding: 10px 12px; font-weight: 600; border-bottom: 1px solid #f0f0f0;
}
.row { cursor: pointer; }
.row.active { background: #e6f4ff; }
</style>
