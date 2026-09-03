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
          <a-badge
            :status="dot(item.id)"
            :text="item.name"
          />
          <template #actions>
            <a-button
              size="small"
              :type="isLive(item.id) ? 'default' : 'link'"
              @click.stop="toggle(item.id)"
            >
              {{ isLive(item.id) ? '断开' : '连接' }}
            </a-button>
            <a-button size="small" type="link" danger @click.stop="remove(item.id)">删</a-button>
          </template>
        </a-list-item>
      </template>
    </a-list>

    <a-modal v-model:open="newOpen" title="新建连接" @ok="submitNew" ok-text="保存">
      <a-form :label-col="{ span: 6 }">
        <a-form-item label="名称"><a-input v-model:value="form.name" /></a-form-item>
        <a-form-item label="传输">
          <a-select v-model:value="form.protocol">
            <a-select-option value="mqtt">mqtt</a-select-option>
            <a-select-option value="mqtts">mqtts</a-select-option>
            <a-select-option value="ws">ws</a-select-option>
            <a-select-option value="wss">wss</a-select-option>
          </a-select>
        </a-form-item>
        <a-form-item label="主机"><a-input v-model:value="form.host" /></a-form-item>
        <a-form-item label="端口"><a-input-number v-model:value="form.port" :min="1" :max="65535" /></a-form-item>
        <a-form-item label="协议版本">
          <a-select v-model:value="form.mqtt_version">
            <a-select-option value="v311">3.1.1</a-select-option>
            <a-select-option value="v50">5.0</a-select-option>
          </a-select>
        </a-form-item>
        <a-form-item label="ClientID"><a-input v-model:value="form.client_id" /></a-form-item>
        <a-form-item label="用户名"><a-input v-model:value="form.username" /></a-form-item>
        <a-form-item label="密码"><a-input-password v-model:value="form.password" /></a-form-item>
        <a-form-item label="CleanSession">
          <a-switch v-model:checked="form.clean_session" />
        </a-form-item>
      </a-form>
    </a-modal>
  </div>
</template>

<script setup lang="ts">
import { computed, reactive, ref } from 'vue'
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

function isLive(id: string) {
  const s = connStore.states[id]
  return s === 'connected' || s === 'reconnecting' || s === 'connecting'
}
function dot(id: string): 'success' | 'processing' | 'default' | 'error' | 'warning' {
  const s = connStore.states[id] as ConnectionState | undefined
  return (
    {
      idle: 'default',
      connecting: 'processing',
      connected: 'success',
      reconnecting: 'warning',
      failed: 'error',
      disconnecting: 'default',
    }[s ?? 'idle'] ?? 'default'
  )
}
function toggle(id: string) {
  if (isLive(id)) connStore.disconnect(id)
  else connStore.connect(id)
}
async function remove(id: string) {
  await connStore.remove(id)
  if (activeId.value === id) activeId.value = ''
}

const newOpen = ref(false)
const form = reactive({
  name: '',
  protocol: 'mqtt',
  host: '127.0.0.1',
  port: 1883,
  mqtt_version: 'v311' as 'v311' | 'v50',
  client_id: '',
  username: '',
  password: '',
  clean_session: true,
})
function openNew() {
  form.name = ''
  form.client_id = 'mqttkit-' + Math.random().toString(16).slice(2, 10)
  form.password = ''
  newOpen.value = true
}
async function submitNew() {
  const conn: Connection = {
    id: crypto.randomUUID(),
    name: form.name || form.host,
    protocol: form.protocol,
    host: form.host,
    port: form.port,
    path: null,
    client_id: form.client_id,
    username: form.username,
    credential_ref: null,
    mqtt_version: form.mqtt_version,
    clean_session: form.clean_session,
    keep_alive: 60,
    tls: { mode: form.protocol === 'mqtts' || form.protocol === 'wss' ? 'tls' : 'none', verify_hostname: true },
    proxy: null,
    last_will: null,
    properties: null,
    auto_connect: false,
    subscriptions: [],
  }
  await connStore.save(conn, form.password || undefined)
  newOpen.value = false
  activeId.value = conn.id
}
</script>

<style scoped>
.conn-list {
  display: flex;
  flex-direction: column;
  height: 100%;
}
.head {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 10px 12px;
  font-weight: 600;
  border-bottom: 1px solid #f0f0f0;
}
.row {
  cursor: pointer;
}
.row.active {
  background: #e6f4ff;
}
</style>
